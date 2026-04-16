#!/usr/bin/env python
"""waveform_dive.py — side-by-side spectrogram + band-envelope diff.

Produces a multi-panel PNG that overlays the original waveform and one
or more encoded versions to localise *what kind* of distortion the
encoder adds — dumped sibilants, shifted formants, thinner presence,
etc. Numbers alone don't answer that; this is a visual dive-in tool.

Usage:
  python waveform_dive.py <original> <encoded1> [encoded2] [encoded3] ... \\
     [--labels orig,frank,classic,sony]
     [--out waveform_dive.png]
     [--focus-band 4000,8000]   # highlight this Hz range in the overlay
     [--start-s 5 --dur-s 8]    # zoom into a specific segment

Outputs (alongside the PNG):
  - terminal summary: per-octave energy delta vs original, for each
    encoded version. Identifies the octaves that are quietest / loudest
    relative to the reference.
  - "vocal clarity" heuristic: ratio of 1-4 kHz RMS to 4-8 kHz RMS.
    Original versus encoded — a bigger ratio means the encoded signal
    lost its Presence band, which the ear interprets as "dumpfer".
"""
import argparse
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
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt


def load_mono(path):
    sr, x = wavfile.read(str(path))
    x = x.astype(np.float32) / 32768.0
    if x.ndim > 1:
        x = x.mean(axis=1)
    return sr, x


def rms_db(x):
    return 20 * np.log10(np.sqrt(np.mean(x**2) + 1e-20))


def band_rms_db(x, sr, lo, hi):
    nyq = sr / 2
    hi = min(hi, nyq - 1)
    sos = sig.butter(4, [lo/nyq, hi/nyq], btype='band', output='sos')
    y = sig.sosfilt(sos, x)
    return rms_db(y), y


def octave_summary(x, sr):
    """Return list of (label, lo_hz, hi_hz, rms_db)."""
    octaves = [
        ('Sub-Bass',   20,   80),
        ('Bass',       80,   250),
        ('Low-Mid',    250,  500),
        ('Mid',        500,  2000),
        ('Upper-Mid',  2000, 4000),
        ('Presence',   4000, 6000),
        ('Brilliance', 6000, 12000),
        ('Air',        12000, 20000),
    ]
    out = []
    for name, lo, hi in octaves:
        rms, _ = band_rms_db(x, sr, lo, hi)
        out.append((name, lo, hi, rms))
    return out


def vocal_clarity_ratio(x, sr):
    """Ratio of voice-fundamental RMS (1-4 kHz) to presence RMS (4-8
    kHz) in dB. Lower is brighter; higher is "dumpfer". The ear hears
    a shift in this ratio very directly."""
    fund, _ = band_rms_db(x, sr, 1000, 4000)
    pres, _ = band_rms_db(x, sr, 4000, 8000)
    return fund - pres


def envelope_per_band(x, sr, bands, hop_s=0.05):
    """Return time axis and per-band envelope dB array (n_bands x n_frames)."""
    hop = int(hop_s * sr)
    win = int(0.05 * sr)
    n_frames = (len(x) - win) // hop
    t = np.arange(n_frames) * hop_s
    env = np.zeros((len(bands), n_frames))
    for i, (_, lo, hi) in enumerate(bands):
        _, y = band_rms_db(x, sr, lo, hi)
        for j in range(n_frames):
            chunk = y[j*hop:j*hop+win]
            env[i, j] = 20 * np.log10(np.sqrt(np.mean(chunk**2) + 1e-20))
    return t, env


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('original')
    ap.add_argument('encoded', nargs='+')
    ap.add_argument('--labels', default=None,
                    help='comma-separated labels, including "orig" first (e.g. orig,frank,classic)')
    ap.add_argument('--out', default='_tmp_at3stats/waveform_dive.png')
    ap.add_argument('--start-s', type=float, default=0.0)
    ap.add_argument('--dur-s', type=float, default=8.0)
    ap.add_argument('--focus-band', default='4000,8000')
    args = ap.parse_args()

    paths = [args.original] + args.encoded
    labels = args.labels.split(',') if args.labels else \
             ['orig'] + [Path(p).stem for p in args.encoded]
    if len(labels) != len(paths):
        print(f"label/path count mismatch: {len(labels)} labels for {len(paths)} files",
              file=sys.stderr)
        sys.exit(1)

    signals = []
    for p in paths:
        sr, x = load_mono(p)
        signals.append((sr, x))
    sr_ref = signals[0][0]
    # align lengths
    n = min(len(x) for _, x in signals)
    signals = [(sr, x[:n]) for sr, x in signals]

    # --------- numbers first, at terminal ---------
    focus_lo, focus_hi = [int(v) for v in args.focus_band.split(',')]

    print(f"\n{'='*72}")
    print(f"waveform_dive  ·  original: {paths[0]}")
    print('='*72)

    # Per-octave table
    octs_per_sig = [octave_summary(x, sr) for sr, x in signals]
    print(f"\n{'Band':<12s} {'Hz':<15s}", end='')
    for l in labels:
        print(f"  {l:>10s}", end='')
    print()
    for i, (name, lo, hi, _) in enumerate(octs_per_sig[0]):
        print(f"{name:<12s} {lo:>5d}-{hi:<9d}", end='')
        ref_db = octs_per_sig[0][i][3]
        for j, octs in enumerate(octs_per_sig):
            val = octs[i][3]
            if j == 0:
                print(f"  {val:>+10.2f}", end='')
            else:
                print(f"  {val-ref_db:>+10.2f}", end='')
        print()
    print("(first column = absolute dBFS; others = Δ vs original)")

    # Vocal-clarity ratio
    print("\n'Vocal-clarity' (1-4 kHz RMS − 4-8 kHz RMS, in dB):")
    print("  Higher = darker / dumpfer (presence band quieter relative to fundamental).")
    for l, (sr, x) in zip(labels, signals):
        ratio = vocal_clarity_ratio(x, sr)
        print(f"  {l:<20s}  {ratio:+.2f} dB")

    # Focus-band energy ratio
    print(f"\nFocus band energy ({focus_lo}-{focus_hi} Hz) RMS vs original:")
    _, ref_f = band_rms_db(signals[0][1], signals[0][0], focus_lo, focus_hi)
    ref_rms = rms_db(ref_f)
    for l, (sr, x) in zip(labels, signals):
        _, y_f = band_rms_db(x, sr, focus_lo, focus_hi)
        delta = rms_db(y_f) - ref_rms
        marker = ''
        if l != labels[0]:
            if delta < -3:
                marker = '  <<< lost a lot'
            elif delta < -1:
                marker = '  <<< quieter'
            elif delta > 1:
                marker = '  >>> louder'
        print(f"  {l:<20s}  {rms_db(y_f):+.2f} dBFS   Δ={delta:+.2f} dB{marker}")

    # --------- plot ---------
    start_s = args.start_s
    dur_s = args.dur_s
    i0 = int(start_s * sr_ref)
    i1 = i0 + int(dur_s * sr_ref)
    i1 = min(i1, n)

    n_sigs = len(signals)
    fig, axes = plt.subplots(n_sigs + 2, 1, figsize=(16, 2.5 * (n_sigs + 2)),
                             sharex=False)

    # Top: spectrograms of each
    for i, ((sr, x), l) in enumerate(zip(signals, labels)):
        ax = axes[i]
        f, t, S = sig.spectrogram(x[i0:i1], sr, nperseg=1024, noverlap=896)
        ax.pcolormesh(t + start_s, f, 10*np.log10(S + 1e-12),
                       cmap='magma', vmin=-80, vmax=-10, shading='auto')
        ax.axhline(focus_lo, color='#58a6ff', alpha=0.5, linestyle='--', lw=0.6)
        ax.axhline(focus_hi, color='#58a6ff', alpha=0.5, linestyle='--', lw=0.6)
        ax.set_ylabel('Hz')
        ax.set_title(f'Spectrogram — {l}')
        ax.set_yscale('log')
        ax.set_ylim(50, 18000)

    # Penultimate: difference spectrogram (first encoded vs original)
    if n_sigs >= 2:
        ax = axes[n_sigs]
        orig = signals[0][1][i0:i1]
        enc  = signals[1][1][i0:i1]
        noise = orig - enc
        f, t, Sn = sig.spectrogram(noise, sr_ref, nperseg=1024, noverlap=896)
        ax.pcolormesh(t + start_s, f, 10*np.log10(Sn + 1e-12),
                       cmap='hot', vmin=-80, vmax=-20, shading='auto')
        ax.set_ylabel('Hz')
        ax.set_title(f'Error spectrum — orig − {labels[1]}   (bright = where noise lives)')
        ax.set_yscale('log')
        ax.set_ylim(50, 18000)

    # Bottom: per-octave envelope overlaid
    ax = axes[-1]
    bands = [('Low-Mid',   250,  500),
             ('Mid',       500,  2000),
             ('Upper-Mid', 2000, 4000),
             ('Presence',  4000, 6000),
             ('Brilliance', 6000, 12000)]
    colours = ['#8b94a5', '#3fb950', '#58a6ff', '#d29922', '#f85149']

    for i, (sr, x) in enumerate(signals):
        t_env, env = envelope_per_band(x[i0:i1], sr, bands)
        ls = '-' if i == 0 else '--'
        for j, (name, _, _) in enumerate(bands):
            ax.plot(t_env + start_s, env[j],
                    color=colours[j], linestyle=ls,
                    alpha=0.9 if i == 0 else 0.6,
                    lw=1.5 if i == 0 else 1.0,
                    label=f'{name} · {labels[i]}' if i < 2 else None)
    ax.set_xlabel('Time (s)')
    ax.set_ylabel('Band RMS (dB)')
    ax.set_title('Per-octave energy envelopes (solid = first signal, dashed = rest)')
    ax.legend(loc='lower right', ncol=2, fontsize=8)
    ax.grid(True, alpha=0.2)

    fig.tight_layout()
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out, dpi=110)
    plt.close(fig)
    print(f"\nwrote {out}")


if __name__ == '__main__':
    main()
