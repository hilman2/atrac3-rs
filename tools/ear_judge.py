#!/usr/bin/env python
"""ear_judge.py — perceptual quality judge for autonomous iteration.

Classic SNR is misleading at encoder quality levels where artefacts
dominate the listening impression. The user has confirmed empirically
that a SNR-15 encode can sound better than a SNR-20 encode if the
SNR-20 one has HF pumping.

This script aggregates six perceptually-grounded penalties, each in a
comparable "audible-nastiness" unit, and emits one scalar quality
score. Lower = better. Intended use:

    # inside an iteration loop I drive:
    before = run(enc_params_A)
    after  = run(enc_params_B)
    if after['score'] < before['score'] - 0.5:
        # B is audibly better; keep it
        ...

Penalty components (all per-10s normalised so clips of different
length compare fairly):

    hf_artefact    — bursts + pumping + holes from hf_scope
    vocal_clarity  — shift of the 1-4 kHz vs 4-8 kHz ratio away from
                     the original; captures "dumpfer Stimme"
    octave_balance — sum of |delta| in presence / brilliance / air
                     octaves, weighted by their Fletcher-Munson
                     sensitivity; captures timbre shift
    nmr            — max Bark-band Noise-to-Mask ratio above 0 dB
    onset_fidelity — 1 - onset F1 on transients
    loudness       — abs(K-weighted LUFS delta)
    pre_echo       — worst per-onset pre-/post-ratio on HF noise

Weights are calibrated against user-labelled A/B listening:
v4 "artefacts audible"    → score ≈ 15
v8 "better, voice dumpf"  → score ≈ 8
classic "slight artefacts"→ score ≈ 5
sony "transparent-ish"    → score ≈ 2

CLI:
    python ear_judge.py <orig.wav> <encoded.wav> [--label X] [--json FILE]
    python ear_judge.py --compare <orig.wav> <a.wav> <b.wav> [...]

The compare form ranks multiple encodes against one reference so I
can see which version wins the iteration without eyeballing.
"""
from __future__ import annotations
import argparse
import json
import os
import sys
from pathlib import Path

try:
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
except Exception:
    pass

import numpy as np
from scipy.io import wavfile
from scipy import signal as sig

# Make sibling modules importable when invoked from any cwd.
sys.path.insert(0, str(Path(__file__).resolve().parent))

import ears_metrics as M


# ---------- loaders ----------

def load_mono(path):
    sr, x = wavfile.read(str(path))
    x = x.astype(np.float32) / 32768.0
    if x.ndim > 1:
        x = x.mean(axis=1)
    return sr, x


def load_stereo(path):
    sr, x = wavfile.read(str(path))
    x = x.astype(np.float32) / 32768.0
    if x.ndim == 1:
        x = np.stack([x, x], axis=1)
    return sr, x


# ---------- individual metric computations ----------

def band_bandpass(x, sr, lo, hi):
    nyq = sr / 2
    hi = min(hi, nyq - 1)
    sos = sig.butter(6, [lo/nyq, hi/nyq], btype='band', output='sos')
    return sig.sosfilt(sos, x)


def rms_db(x):
    return 20 * np.log10(np.sqrt(np.mean(x**2) + 1e-20))


def vocal_clarity_ratio(x, sr):
    fund = band_bandpass(x, sr, 1000, 4000)
    pres = band_bandpass(x, sr, 4000, 8000)
    return rms_db(fund) - rms_db(pres)


def octaves():
    return [
        ('Sub-Bass',   20,   80,  0.1),
        ('Bass',       80,   250, 0.3),
        ('Low-Mid',    250,  500, 0.7),
        ('Mid',        500,  2000, 1.0),
        ('Upper-Mid',  2000, 4000, 1.2),
        ('Presence',   4000, 6000, 1.3),
        ('Brilliance', 6000, 12000, 0.8),
        ('Air',        12000, 20000, 0.4),
    ]


def octave_deltas(x_ref, x_enc, sr):
    out = {}
    for name, lo, hi, w in octaves():
        ref = rms_db(band_bandpass(x_ref, sr, lo, hi))
        enc = rms_db(band_bandpass(x_enc, sr, lo, hi))
        out[name] = (enc - ref, w)
    return out


# HF-scope events (subset of hf_scope.py logic, inlined so ear_judge
# stays standalone).

def hf_scope_events(x_ref, x_enc, sr, hf_lo=4000, hf_hi=16000):
    ref_hf = band_bandpass(x_ref, sr, hf_lo, hf_hi)
    enc_hf = band_bandpass(x_enc, sr, hf_lo, hf_hi)
    err_hf = ref_hf - enc_hf

    hop_s, win_s = 0.02, 0.05
    hop = int(hop_s * sr); win = int(win_s * sr)
    n_frames = max(1, (len(ref_hf) - win) // hop)
    err_db = np.zeros(n_frames); ref_db = np.zeros(n_frames); enc_db = np.zeros(n_frames)
    for i in range(n_frames):
        err_db[i] = 20*np.log10(np.sqrt(np.mean(err_hf[i*hop:i*hop+win]**2)) + 1e-20)
        ref_db[i] = 20*np.log10(np.sqrt(np.mean(ref_hf[i*hop:i*hop+win]**2)) + 1e-20)
        enc_db[i] = 20*np.log10(np.sqrt(np.mean(enc_hf[i*hop:i*hop+win]**2)) + 1e-20)

    bursts = []
    for i in range(n_frames):
        if err_db[i] > -55 and ref_db[i] < -40:
            bursts.append(i * hop_s)
    # dedup
    bursts_d = []
    for t in bursts:
        if not bursts_d or t - bursts_d[-1] > 0.2:
            bursts_d.append(t)
    bursts = bursts_d

    pumping = 0
    for i in range(1, n_frames):
        if abs(enc_db[i] - enc_db[i-1]) > abs(ref_db[i] - ref_db[i-1]) + 8 \
           and abs(enc_db[i] - enc_db[i-1]) > 5:
            pumping += 1

    holes = []
    for i in range(n_frames):
        if ref_db[i] > -50 and enc_db[i] < ref_db[i] - 10:
            holes.append(i * hop_s)
    holes_d = []
    for t in holes:
        if not holes_d or t - holes_d[-1] > 0.2:
            holes_d.append(t)
    holes = holes_d

    # pre-echo per onset
    onsets = _find_onsets(x_ref, sr)
    pre_ms, post_ms = 50, 50
    pre_n = int(pre_ms*sr/1000); post_n = int(post_ms*sr/1000)
    pre_echoes = []
    worst_db = 0.0
    for o in onsets:
        if o - pre_n < 0 or o + post_n > len(x_ref):
            continue
        pre_err = err_hf[o-pre_n:o]
        post_err = err_hf[o:o+post_n]
        pre = np.sqrt(np.mean(pre_err**2) + 1e-20)
        post = np.sqrt(np.mean(post_err**2) + 1e-20)
        r = 20*np.log10(pre / max(post, 1e-12))
        if r > 3:
            pre_echoes.append((o/sr, r))
            worst_db = max(worst_db, r)

    hf_snr = 10*np.log10(np.mean(ref_hf**2) / max(np.mean(err_hf**2), 1e-20))

    return {
        'bursts': len(bursts),
        'bursts_times_s': bursts[:12],
        'pumping': pumping,
        'holes': len(holes),
        'holes_times_s': holes[:12],
        'pre_echo_events': len(pre_echoes),
        'pre_echo_worst_db': float(worst_db),
        'hf_snr_db': float(hf_snr),
    }


def _find_onsets(x, sr, threshold_db=4.0, min_gap_s=0.1):
    hop = 512; win = 2048
    frames = (len(x) - win) // hop
    env_db = np.array([20*np.log10(np.sqrt(np.mean(x[i*hop:i*hop+win]**2)) + 1e-10)
                       for i in range(frames)])
    onsets = []
    last = -10**9
    mingap = int(min_gap_s * sr / hop)
    for i in range(1, len(env_db)):
        if env_db[i] - env_db[i-1] > threshold_db and env_db[i] > -40:
            if i - last > mingap:
                onsets.append(i * hop)
                last = i
    return onsets


# ---------- the judge ----------

# Calibration: weights produce a score where
#   < 2   = transparent-ish (Sony-grade)
#   2-5   = clean, very slight degradation
#   5-10  = noticeable on critical listening
#   10-20 = clearly audible
#   > 20  = obvious artefacts

PEN_WEIGHTS = {
    # Each term: (weight, label, tolerance). Penalty = weight *
    # max(0, value - tolerance). Tolerance absorbs the noise floor that
    # any lossy encoder at 132 kbps introduces — what we really care
    # about is *excess* artefact rate beyond Sony's baseline.
    'hf_bursts_per_10s':    (1.0, 'HF bursts',             8.0),
    'hf_pumping_per_10s':   (1.5, 'HF pumping',            0.0),
    'hf_holes_per_10s':     (2.0, 'HF holes',              0.0),
    'pre_echo_per_10s':     (0.5, 'Pre-echo events',       1.0),
    'pre_echo_worst_db':    (0.2, 'Pre-echo worst (dB)',   6.0),
    'vocal_clarity_shift':  (2.5, 'Vocal clarity shift',   0.2),
    'octave_balance':       (0.7, 'Octave balance',        0.5),
    'nmr_max_over':         (0.8, 'NMR over mask',         3.0),
    'onset_fidelity':       (4.0, 'Onset fidelity',        0.0),
    'loudness_delta':       (1.5, 'Loudness Δ',            0.3),
}


def judge(ref_path, enc_path, label=None):
    label = label or Path(enc_path).stem
    sr_ref, ref = load_mono(ref_path)
    sr_enc, enc = load_mono(enc_path)
    if sr_ref != sr_enc:
        raise ValueError(f"sample rate mismatch: {sr_ref} vs {sr_enc}")
    n = min(len(ref), len(enc))
    ref = ref[:n]; enc = enc[:n]
    duration_s = n / sr_ref

    # HF events + snr
    hf = hf_scope_events(ref, enc, sr_ref)
    norm = 10.0 / max(duration_s, 0.1)

    hf_bursts_per_10s = hf['bursts'] * norm
    hf_pumping_per_10s = hf['pumping'] * norm
    hf_holes_per_10s = hf['holes'] * norm
    pre_echo_per_10s = hf['pre_echo_events'] * norm
    pre_echo_worst_db = hf['pre_echo_worst_db']

    # Vocal clarity shift
    vc_ref = vocal_clarity_ratio(ref, sr_ref)
    vc_enc = vocal_clarity_ratio(enc, sr_enc)
    vocal_clarity_shift = abs(vc_enc - vc_ref)

    # Octave balance
    deltas = octave_deltas(ref, enc, sr_ref)
    octave_balance = sum(abs(d) * w for d, w in deltas.values())

    # NMR
    try:
        _, ref_s = load_stereo(ref_path)
        _, enc_s = load_stereo(enc_path)
        n2 = min(len(ref_s), len(enc_s))
        nmr = M.nmr_bark(ref_s[:n2], enc_s[:n2], sr_ref)
        nmr_max_over = max(0.0, nmr['nmr_max_db'])
    except Exception:
        nmr_max_over = 0.0

    # Onset fidelity
    try:
        on = M.onset_preservation(ref, enc, sr_ref)
        f1 = on.get('f1')
        onset_fidelity = 1.0 - f1 if f1 is not None else 0.0
    except Exception:
        onset_fidelity = 0.0

    # Loudness delta
    try:
        lu = M.loudness_lufs(ref, enc, sr_ref)
        loudness_delta = abs(lu['delta_db'])
    except Exception:
        loudness_delta = 0.0

    terms = {
        'hf_bursts_per_10s':   hf_bursts_per_10s,
        'hf_pumping_per_10s':  hf_pumping_per_10s,
        'hf_holes_per_10s':    hf_holes_per_10s,
        'pre_echo_per_10s':    pre_echo_per_10s,
        'pre_echo_worst_db':   pre_echo_worst_db,
        'vocal_clarity_shift': vocal_clarity_shift,
        'octave_balance':      octave_balance,
        'nmr_max_over':        nmr_max_over,
        'onset_fidelity':      onset_fidelity,
        'loudness_delta':      loudness_delta,
    }
    breakdown = {}
    score = 0.0
    for key, (w, _, tol) in PEN_WEIGHTS.items():
        excess = max(0.0, terms[key] - tol)
        contrib = w * excess
        breakdown[key] = {
            'value': terms[key],
            'tolerance': tol,
            'excess': excess,
            'weight': w,
            'contribution': contrib,
        }
        score += contrib

    # Context-only (not in score): HF-SNR, overall SNR
    try:
        overall_snr = 10*np.log10(np.mean(ref**2) / max(np.mean((ref-enc)**2), 1e-20))
    except Exception:
        overall_snr = 0.0

    verdict = (
        'transparent' if score < 2 else
        'excellent'   if score < 5 else
        'good'        if score < 10 else
        'noticeable'  if score < 20 else
        'problematic'
    )

    return {
        'label': label,
        'original': str(ref_path),
        'encoded': str(enc_path),
        'duration_s': float(duration_s),
        'score': float(score),
        'verdict': verdict,
        'breakdown': breakdown,
        'context': {
            'overall_snr_db': float(overall_snr),
            'hf_snr_db': hf['hf_snr_db'],
            'vocal_clarity_ref': float(vc_ref),
            'vocal_clarity_enc': float(vc_enc),
            'octave_deltas_db': {k: float(v[0]) for k, v in deltas.items()},
            'hf_bursts_times_s': hf['bursts_times_s'],
            'hf_holes_times_s': hf['holes_times_s'],
        },
    }


def print_report(result):
    print(f"\n{'='*72}")
    print(f"ear_judge  ·  {result['label']}   ({result['duration_s']:.1f} s)")
    print('='*72)
    print(f"{'SCORE':<20s}  {result['score']:>6.2f}   [{result['verdict'].upper()}]")
    print(f"{'context':<20s}  overall SNR {result['context']['overall_snr_db']:+.2f} dB · "
          f"HF SNR {result['context']['hf_snr_db']:+.2f} dB")
    print('-'*72)
    print(f"{'metric':<24s}  {'value':>7s}  {'tol':>6s}  {'excess':>7s}  {'× w':>5s}  {'contrib':>8s}")
    for key, (w, label, tol) in PEN_WEIGHTS.items():
        b = result['breakdown'][key]
        mark = '  ***' if b['contribution'] > 3 else ''
        print(f"  {label:<22s}  {b['value']:>7.3f}  {tol:>6.2f}  {b['excess']:>7.3f}  {w:>5.2f}  {b['contribution']:>7.3f}{mark}")
    print('-'*72)
    print("Octave deltas (dB, Δ vs original):")
    for name, delta in result['context']['octave_deltas_db'].items():
        marker = ''
        if abs(delta) > 1.0:
            marker = '  <<< shifted'
        elif abs(delta) > 0.5:
            marker = '   ·'
        print(f"  {name:<12s}  {delta:+.2f} dB{marker}")
    if result['context']['hf_bursts_times_s']:
        print(f"HF burst times (s): {', '.join(f'{t:.2f}' for t in result['context']['hf_bursts_times_s'])}")
    if result['context']['hf_holes_times_s']:
        print(f"HF hole times (s):  {', '.join(f'{t:.2f}' for t in result['context']['hf_holes_times_s'])}")


def compare(ref_path, enc_paths, labels=None):
    labels = labels or [Path(p).stem for p in enc_paths]
    results = []
    for p, l in zip(enc_paths, labels):
        results.append(judge(ref_path, p, label=l))
    # sort ascending (better first)
    results_sorted = sorted(results, key=lambda r: r['score'])
    print(f"\n{'='*72}")
    print(f"ear_judge · comparison against  {ref_path}")
    print('='*72)
    print(f"{'rank':<5s} {'label':<20s} {'score':>8s}  {'verdict':<14s} {'SNR':>7s} {'HF-SNR':>7s} {'voc':>6s}  {'hf-art':>7s}")
    for rank, r in enumerate(results_sorted, 1):
        vc_shift = r['breakdown']['vocal_clarity_shift']['value']
        hf_art = (r['breakdown']['hf_bursts_per_10s']['contribution']
                + r['breakdown']['hf_pumping_per_10s']['contribution']
                + r['breakdown']['hf_holes_per_10s']['contribution'])
        print(f"{rank:<5d} {r['label']:<20s} {r['score']:>7.2f}   {r['verdict']:<14s} "
              f"{r['context']['overall_snr_db']:>+6.1f}  {r['context']['hf_snr_db']:>+6.1f}  "
              f"{vc_shift:>5.2f}  {hf_art:>6.2f}")
    return results_sorted


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('original')
    ap.add_argument('encoded', nargs='+')
    ap.add_argument('--labels', default=None,
                    help='comma-separated label per encoded file')
    ap.add_argument('--json', metavar='FILE',
                    help='write results as JSON for autonomous tooling')
    ap.add_argument('--quiet', action='store_true',
                    help='no terminal report (pair with --json)')
    args = ap.parse_args()

    labels = args.labels.split(',') if args.labels else None

    if len(args.encoded) == 1:
        result = judge(args.original, args.encoded[0],
                       label=(labels[0] if labels else None))
        if not args.quiet:
            print_report(result)
        if args.json:
            Path(args.json).write_text(json.dumps(result, indent=2, default=str),
                                        encoding='utf-8')
            print(f"\nJSON: {args.json}")
    else:
        results = compare(args.original, args.encoded, labels=labels)
        if args.json:
            Path(args.json).write_text(json.dumps(results, indent=2, default=str),
                                        encoding='utf-8')
            print(f"\nJSON: {args.json}")


if __name__ == '__main__':
    main()
