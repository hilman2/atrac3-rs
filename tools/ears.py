#!/usr/bin/env python
"""ears.py — vereinheitlichte Test-Suite für atrac3-rs.

Ein Input-WAV rein, volle professionelle Metriken raus. Pipeline-Stufen:

  1. Encode mit atrac3-rs proto-at3 (VLC, k132 default)
  2. Decode mit Sony psp_at3tool.exe
  3. Referenzmessungen gegen das Original:
       - digital_ears   : SNR, HF-Env-Corr, Pre-Echo, Per-Band SNR,
                          Artefakt-Lokalisation, Spektrogramm-Diff
       - stereo_ears    : Mid/Side SNR, L-R-Korrelation, Stereo-Width
       - loudness       : K-weighted LUFS (BS.1770 simplified)
       - spectral       : Centroid, Rolloff, Flatness, Flux
       - dynamic_range  : Crest-Factor + DR14
       - nmr_bark       : Noise-to-Mask-Ratio auf 24 Bark-Bändern
       - onsets         : Transient-Preservation (Precision/Recall/F1)
       - thd_plus_n     : THD+N auf dominanter stationärer Frequenz
  4. Optional: Sony-Referenz 3-fach Vergleich (--sony)
  5. Ausgabe: Konsole + optional JSON + optional HTML-Report

CLAUDE.md-Fallen werden automatisch gehandhabt (--coding-mode vlc, k132).

Usage:
  python ears.py testsong/Crystallize90.wav
  python ears.py testsong/HateMe.wav --sony --html
  python ears.py my.wav --json out.json --html report.html
"""
import argparse
import json
import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path

try:
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
except Exception:
    pass

import numpy as np
from scipy.io import wavfile

# Make sibling modules importable regardless of cwd.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from digital_ears import measure
from stereo_ears import stereo_analysis
import ears_metrics as M
import ears_report

TOOLS_DIR = Path(__file__).resolve().parent         # atrac3-rs/tools/
REPO_ROOT = TOOLS_DIR.parent                         # atrac3-rs/
AT3CMP = REPO_ROOT / 'target' / 'release' / 'at3cmp.exe'
# Locate psp_at3tool.exe: env var wins, otherwise search cwd and the
# usual sibling of the repo (psp_at3tool is typically kept alongside the
# test assets rather than inside the encoder repo).
_PSP_CANDIDATES = [
    Path(p) for p in [
        os.environ.get('PSP_AT3TOOL', ''),
        str(Path.cwd() / 'psp_at3tool.exe'),
        str(REPO_ROOT.parent / 'psp_at3tool.exe'),
        str(TOOLS_DIR / 'psp_at3tool.exe'),
    ] if p
]
PSP_TOOL = next((p for p in _PSP_CANDIDATES if p.exists()), _PSP_CANDIDATES[1])
TMP_DIR = Path.cwd() / '_tmp_at3stats'              # output lands next to inputs


# ---------- tool invocations ----------

def run(cmd):
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.returncode != 0:
        raise RuntimeError(
            f"Command failed ({result.returncode}): {' '.join(map(str, cmd))}\n"
            f"stderr: {result.stderr.strip()}\nstdout: {result.stdout.strip()}"
        )
    return result.stdout


def ensure_tools():
    missing = [str(p) for p in (AT3CMP, PSP_TOOL) if not p.exists()]
    if missing:
        hint = ""
        if not AT3CMP.exists():
            hint = ("\nHinweis: at3cmp.exe nicht gefunden. Build mit:\n"
                    "  cd atrac3-rs && cargo build --release")
        raise FileNotFoundError(f"Fehlende Tools: {missing}{hint}")


def atrac3rs_encode(input_wav, out_at3, bitrate='k132', frames=9999):
    run([
        str(AT3CMP), 'proto-at3',
        '--input', str(input_wav),
        '--output', str(out_at3),
        '--frames', str(frames),
        '--coding-mode', 'vlc',
        '--bitrate', bitrate,
    ])


def sony_encode(input_wav, out_at3, bitrate_kbps=132):
    out = Path(out_at3)
    if out.exists():
        out.unlink()
    run([str(PSP_TOOL), '-e', '-br', str(bitrate_kbps), str(input_wav), str(out_at3)])


def psp_decode(in_at3, out_wav):
    # psp_at3tool.exe refuses to overwrite; remove any stale output first.
    out = Path(out_wav)
    if out.exists():
        out.unlink()
    run([str(PSP_TOOL), '-d', str(in_at3), str(out_wav)])


# ---------- metric sweep ----------

def _load_wavs(ref_path, test_path):
    sr_r, r = wavfile.read(str(ref_path))
    sr_t, t = wavfile.read(str(test_path))
    if sr_r != sr_t:
        raise ValueError(f"sample rates differ: ref={sr_r}, test={sr_t}")
    r = r.astype(np.float32) / 32768.0
    t = t.astype(np.float32) / 32768.0
    n = min(len(r), len(t))
    return sr_r, r[:n], t[:n]


def collect_metrics(ref_path, test_path, label, with_stereo=True):
    """Run all metric modules against a ref/test pair and return a dict."""
    # Console output via existing digital_ears / stereo_ears (keeps the
    # familiar console report unchanged for humans).
    de = measure(str(ref_path), str(test_path), label)
    st = stereo_analysis(str(ref_path), str(test_path), label) if with_stereo else None

    # Now gather the professional metrics directly.
    sr, ref, test = _load_wavs(ref_path, test_path)

    try:    loud = M.loudness_lufs(ref, test, sr)
    except Exception as e: loud = {'error': str(e)}
    try:    spec = M.spectral_features(ref, test, sr)
    except Exception as e: spec = {'error': str(e)}
    try:    dyn = M.dynamic_range(ref, test, sr)
    except Exception as e: dyn = {'error': str(e)}
    try:    nmr = M.nmr_bark(ref, test, sr)
    except Exception as e: nmr = {'error': str(e)}
    try:    ons = M.onset_preservation(ref, test, sr)
    except Exception as e: ons = {'error': str(e)}
    try:    thd = M.thd_plus_n(ref, test, sr)
    except Exception as e: thd = {'error': str(e)}

    return {
        'digital_ears': de,
        'stereo':       st,
        'loudness':     loud,
        'spectral':     spec,
        'dynamic':      dyn,
        'nmr':          nmr,
        'onsets':       ons,
        'thd':          thd,
    }


# ---------- console summary ----------

def print_pro_metrics(block, heading):
    print(f"\n{'='*70}")
    print(f"  {heading}")
    print('='*70)
    lu = block.get('loudness', {})
    if 'error' not in lu:
        print(f"Loudness (K-weighted):       ref={lu.get('ref_lufs', 0):+6.2f} LUFS   "
              f"test={lu.get('test_lufs', 0):+6.2f} LUFS   Δ={lu.get('delta_db', 0):+.2f} dB")
    sp = block.get('spectral', {})
    if 'error' not in sp:
        print(f"Spectral centroid:           shift={sp.get('centroid_shift_hz', 0):+7.1f} Hz  "
              f"(ref {sp.get('centroid_ref_hz', 0):.0f} → test {sp.get('centroid_test_hz', 0):.0f})")
        print(f"Spectral rolloff 85%:        shift={sp.get('rolloff_shift_hz', 0):+7.1f} Hz")
        print(f"Spectral flux ratio:         {sp.get('flux_ratio', 0):.3f}   "
              f"flatness Δ={sp.get('flatness_delta', 0):+.4f}")
    dr = block.get('dynamic', {})
    if 'error' not in dr:
        print(f"Crest factor:                ref={dr.get('crest_ref_db', 0):+.2f} dB  "
              f"test={dr.get('crest_test_db', 0):+.2f} dB  Δ={dr.get('crest_delta_db', 0):+.2f} dB")
        print(f"DR14:                        ref={dr.get('dr14_ref', 0):.2f}   "
              f"test={dr.get('dr14_test', 0):.2f}   Δ={dr.get('dr14_delta', 0):+.2f}")
    nm = block.get('nmr', {})
    if 'error' not in nm:
        print(f"NMR (Bark-bands):            max={nm.get('nmr_max_db', 0):+.2f} dB   "
              f"mean={nm.get('nmr_mean_db', 0):+.2f} dB   "
              f"({'unter' if nm.get('nmr_max_db', 0) < 0 else 'über'} Masking)")
    on = block.get('onsets', {})
    if 'error' not in on:
        print(f"Transient preservation:      precision={on.get('precision', 0):.3f}   "
              f"recall={on.get('recall', 0):.3f}   F1={on.get('f1', 0):.3f}   "
              f"({on.get('matched', 0)}/{on.get('ref_count', 0)} onsets matched, "
              f"{on.get('phantom', 0)} phantom)")
    th = block.get('thd', {})
    if 'error' not in th and th.get('fundamental_hz') is not None:
        print(f"THD+N on {th['fundamental_hz']:.1f} Hz:    "
              f"{th.get('thd_db', 0):+.2f} dB   (signal {th.get('signal_dbfs', 0):+.1f} dBFS)")


def print_passfail_summary(block):
    """Print a single-line pass/fail rollup."""
    de = block.get('digital_ears', {})
    lu = block.get('loudness', {})
    nm = block.get('nmr', {})
    on = block.get('onsets', {})
    sp = block.get('spectral', {})
    checks = [
        ('SNR',             de.get('snr'),               'snr'),
        ('HF-Corr',         de.get('hf_corr'),           'hf_corr'),
        ('Pre-Echo',        de.get('pe_worst'),          'pe_worst'),
        ('NMR-max',         nm.get('nmr_max_db'),        'nmr_max_db'),
        ('Loudness-Δ',      lu.get('delta_db'),          'loudness_delta'),
        ('Onset-F1',        on.get('f1'),                'onset_f1'),
        ('Centroid-shift',  sp.get('centroid_shift_hz'), 'centroid_shift_hz'),
    ]
    pills = []
    any_fail = any_warn = False
    for name, v, key in checks:
        rating = ears_report._rate(key, v)
        if   rating == 'good': marker = 'PASS'
        elif rating == 'warn': marker = 'WARN'; any_warn = True
        elif rating == 'fail': marker = 'FAIL'; any_fail = True
        else:                  marker = '—'
        pills.append(f"{name}={marker}")
    verdict = 'FAIL' if any_fail else ('WARN' if any_warn else 'PASS')
    print(f"\n{'='*70}")
    print(f"  PASS/FAIL ROLLUP: {verdict}   ({' · '.join(pills)})")
    print('='*70)


# ---------- main ----------

def main():
    ap = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    ap.add_argument('input_wav', help='Original-WAV (Referenz-Signal)')
    ap.add_argument('--sony', action='store_true',
                    help='Sony-Referenz zusätzlich encoden und vergleichen')
    ap.add_argument('--bitrate', default='k132',
                    help='at3cmp bitrate-flag (default: k132)')
    ap.add_argument('--bitrate-kbps', type=int, default=132,
                    help='Sony-bitrate in kbps (default: 132)')
    ap.add_argument('--label', default=None,
                    help='Label für die Ausgabe (default: Dateiname)')
    ap.add_argument('--no-stereo', action='store_true', help='Stereo-Analyse überspringen')
    ap.add_argument('--no-plot', action='store_true', help='Spektrogramm-PNG überspringen')
    ap.add_argument('--no-artifacts', action='store_true',
                    help='Artefakt-Lokalisation überspringen')
    ap.add_argument('--json', metavar='FILE',
                    help='Alle Metriken nach JSON schreiben')
    ap.add_argument('--html', metavar='FILE', nargs='?', const='',
                    help='HTML-Report schreiben (leer = Auto-Pfad)')
    ap.add_argument('--quick', action='store_true',
                    help='Kurzmodus: nur Kernmetriken, keine Artefakte/Plot')
    args = ap.parse_args()

    if args.quick:
        args.no_plot = True
        args.no_artifacts = True

    ensure_tools()
    TMP_DIR.mkdir(exist_ok=True)

    input_wav = Path(args.input_wav).resolve()
    if not input_wav.exists():
        print(f"ERROR: input_wav nicht gefunden: {input_wav}", file=sys.stderr)
        sys.exit(1)

    stem = input_wav.stem
    label = args.label or stem

    ours_at3 = TMP_DIR / f'{stem}_ours.at3'
    ours_wav = TMP_DIR / f'{stem}_ours.wav'
    sony_at3 = TMP_DIR / f'{stem}_sony.at3'
    sony_wav = TMP_DIR / f'{stem}_sony.wav'
    spectro_png = TMP_DIR / f'{stem}_ours_spectrogram.png'

    if args.no_plot:
        os.environ['DIGITAL_EARS_NO_PLOT'] = '1'
    else:
        os.environ.pop('DIGITAL_EARS_NO_PLOT', None)
    if args.no_artifacts:
        os.environ['DIGITAL_EARS_NO_ARTIFACTS'] = '1'
    else:
        os.environ.pop('DIGITAL_EARS_NO_ARTIFACTS', None)

    total_steps = 3 if args.sony else 2
    print(f"[1/{total_steps}] atrac3-rs encode ({args.bitrate}, VLC) …")
    atrac3rs_encode(input_wav, ours_at3, args.bitrate)
    print(f"[2/{total_steps}] psp_at3tool decode …")
    psp_decode(ours_at3, ours_wav)
    if args.sony:
        print(f"[3/{total_steps}] Sony-Referenz encode+decode ({args.bitrate_kbps} kbps) …")
        sony_encode(input_wav, sony_at3, args.bitrate_kbps)
        psp_decode(sony_at3, sony_wav)

    # ---- run metric sweeps ----
    ours = collect_metrics(input_wav, ours_wav, label, with_stereo=not args.no_stereo)
    sony = None
    if args.sony:
        sony = collect_metrics(input_wav, sony_wav, f"{label} | Sony ref",
                               with_stereo=not args.no_stereo)

    # ---- extended console metrics ----
    print_pro_metrics(ours, f"Professionelle Metriken — {label}")
    if sony:
        print_pro_metrics(sony, f"Professionelle Metriken — Sony-Referenz")

    # ---- ours vs sony (bit-level match) ----
    if args.sony:
        print(f"\n{'#'*72}")
        print(f"  OURS vs SONY — bit-exakter Abgleich")
        print('#'*72)
        measure(str(sony_wav), str(ours_wav), f"{label} | ours-vs-sony")

    # ---- pass/fail rollup ----
    print_passfail_summary(ours)

    # ---- results dict (JSON + HTML) ----
    results = {
        'label':           label,
        'input':           str(input_wav),
        'bitrate':         args.bitrate,
        'bitrate_kbps':    args.bitrate_kbps,
        'timestamp':       datetime.now().isoformat(timespec='seconds'),
        'sony_compared':   args.sony,
        'ours':            _serialisable(ours),
        'sony':            _serialisable(sony) if sony else None,
        'spectrogram_png': str(spectro_png) if spectro_png.exists() else None,
    }

    if args.json:
        Path(args.json).write_text(
            json.dumps(results, indent=2, default=str), encoding='utf-8',
        )
        print(f"\nJSON: {args.json}")

    if args.html is not None:
        html_path = args.html or str(TMP_DIR / f'{stem}_ears_report.html')
        ears_report.generate(results, html_path)
        print(f"HTML: {html_path}")

    # ---- artefact listing (explicit, no wildcard) ----
    produced = [ours_at3, ours_wav]
    if args.sony:
        produced += [sony_at3, sony_wav]
    if spectro_png.exists():
        produced.append(spectro_png)
    print(f"\nArtefakte in {TMP_DIR}:")
    for p in produced:
        if p.exists():
            print(f"  {p.name}")


def _serialisable(obj):
    """Strip np/tuple weirdness so json.dumps works. Bands-dict in
    digital_ears has tuple values which JSON can't serialise directly."""
    if obj is None:
        return None
    if isinstance(obj, dict):
        return {k: _serialisable(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return [_serialisable(v) for v in obj]
    if isinstance(obj, (np.floating, np.integer)):
        return float(obj)
    if isinstance(obj, np.ndarray):
        return obj.tolist()
    return obj


if __name__ == '__main__':
    main()
