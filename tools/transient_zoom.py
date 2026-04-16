#!/usr/bin/env python
"""transient_zoom.py — zoom on a transient to find pre/post "holes".

A "hole" around a transient is reduced SIGNAL level in the few frames
before/after the attack, not increased noise. The existing Pre-Echo
detector is blind to this (it compares pre-region *noise* RMS to
post-region *noise* RMS — both can fall together and yet the hole
stays audible as a gap in the music around the hit).

This tool:
  1. Finds the N strongest transients in the ORIGINAL via the
     artifact_finder onset detector.
  2. For each, overlays time-domain RMS envelopes and per-octave
     energies of original vs encoded in a ±200 ms window.
  3. Reports, per onset, the "hole depth" = max drop of encoded
     energy below reference within ±150 ms of the attack (excluding
     the attack sample itself).

Usage:
  python tools/transient_zoom.py <original> <encoded> [--label X] \\
         [--onsets 3] [--window-ms 200] [--out hole_report.png]
"""
import argparse
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


def bandpass(x, sr, lo, hi):
    nyq = sr / 2
    hi = min(hi, nyq - 1)
    sos = sig.butter(6, [lo/nyq, hi/nyq], btype='band', output='sos')
    return sig.sosfilt(sos, x)


def find_strongest_onsets(x, sr, n_max=3, min_jump_db=2.0):
    """Find the N loudest transient-onsets by energy-jump."""
    hop, win = 256, 1024
    frames = (len(x) - win) // hop
    env = np.array([np.sqrt(np.mean(x[i*hop:i*hop+win]**2)) for i in range(frames)])
    env_db = 20 * np.log10(env + 1e-10)
    candidates = []
    for i in range(2, len(env_db)):
        jump = env_db[i] - env_db[i-1]
        if jump > min_jump_db and env_db[i] > -40:
            candidates.append((i * hop, jump, env_db[i]))
    # Merge close-in-time peaks (within 150 ms)
    merge_gap = int(0.15 * sr)
    candidates.sort(key=lambda c: -c[1])
    picked = []
    for pos, jump, lvl in candidates:
        if all(abs(pos - p) > merge_gap for p, _, _ in picked):
            picked.append((pos, jump, lvl))
        if len(picked) >= n_max:
            break
    picked.sort(key=lambda c: c[0])  # chronological
    return picked


def envelope_db(x, sr, hop_ms=2.0, win_ms=5.0):
    """Short-window RMS envelope in dB."""
    hop = int(hop_ms * sr / 1000)
    win = int(win_ms * sr / 1000)
    n = max(1, (len(x) - win) // hop)
    out = np.zeros(n)
    for i in range(n):
        seg = x[i*hop:i*hop+win]
        out[i] = 20 * np.log10(np.sqrt(np.mean(seg**2)) + 1e-10)
    t = np.arange(n) * hop_ms / 1000.0
    return t, out


def hole_depth_db(ref_env, enc_env, t, attack_s, guard_ms=5.0, window_ms=150.0):
    """Max dB drop of encoded below reference in ±window_ms of attack,
    excluding a narrow guard band ±guard_ms around the attack itself
    (where the encoder is allowed to concentrate bits)."""
    lo = attack_s - window_ms / 1000.0
    hi = attack_s + window_ms / 1000.0
    g_lo = attack_s - guard_ms / 1000.0
    g_hi = attack_s + guard_ms / 1000.0
    mask = ((t >= lo) & (t <= hi)) & ~((t >= g_lo) & (t <= g_hi))
    if not mask.any():
        return 0.0, 0.0
    delta = enc_env[mask] - ref_env[mask]
    worst = float(delta.min())  # most negative = deepest hole
    worst_t = float(t[mask][int(np.argmin(delta))])
    return worst, worst_t


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('original')
    ap.add_argument('encoded')
    ap.add_argument('--label', default=None)
    ap.add_argument('--onsets', type=int, default=3)
    ap.add_argument('--window-ms', type=float, default=200.0)
    ap.add_argument('--out', default=None)
    args = ap.parse_args()

    label = args.label or Path(args.encoded).stem
    sr, ref = load_mono(args.original)
    _,  enc = load_mono(args.encoded)
    n = min(len(ref), len(enc))
    ref = ref[:n]; enc = enc[:n]

    onsets = find_strongest_onsets(ref, sr, n_max=args.onsets)
    if not onsets:
        print("no transients found.")
        return

    print(f"\n{'='*72}")
    print(f"transient_zoom  ·  {label}")
    print('='*72)
    print(f"{'#':<3s}  {'time':>7s}  {'attack':>7s}  {'full-hole':>9s}  {'HF-hole':>7s}  {'Mid-hole':>8s}")

    # Plot
    n_on = len(onsets)
    fig, axes = plt.subplots(n_on, 2, figsize=(14, 3.0 * n_on), squeeze=False)

    hole_summary = []
    for i, (pos, jump, lvl) in enumerate(onsets):
        attack_s = pos / sr
        half_w = args.window_ms / 1000.0
        lo = max(0, pos - int(half_w * sr))
        hi = min(len(ref), pos + int(half_w * sr))

        ref_seg = ref[lo:hi]
        enc_seg = enc[lo:hi]
        t_seg = (np.arange(len(ref_seg)) - (pos - lo)) / sr * 1000.0  # ms, 0 at attack

        # Overall envelope
        t_env, ref_env = envelope_db(ref_seg, sr)
        _,    enc_env = envelope_db(enc_seg, sr)
        t_env_ms = (t_env * 1000.0) - (pos - lo) / sr * 1000.0

        full_hole, full_hole_t = hole_depth_db(ref_env, enc_env, t_env, (pos - lo) / sr)

        # HF band (4-12 kHz, snare sizzle)
        ref_hf = bandpass(ref_seg, sr, 4000, min(sr/2 - 1, 12000))
        enc_hf = bandpass(enc_seg, sr, 4000, min(sr/2 - 1, 12000))
        _, ref_hf_env = envelope_db(ref_hf, sr)
        _, enc_hf_env = envelope_db(enc_hf, sr)
        hf_hole, _ = hole_depth_db(ref_hf_env, enc_hf_env, t_env, (pos - lo) / sr)

        # Mid band (300 Hz - 2 kHz, snare body)
        ref_mid = bandpass(ref_seg, sr, 300, 2000)
        enc_mid = bandpass(enc_seg, sr, 300, 2000)
        _, ref_mid_env = envelope_db(ref_mid, sr)
        _, enc_mid_env = envelope_db(enc_mid, sr)
        mid_hole, _ = hole_depth_db(ref_mid_env, enc_mid_env, t_env, (pos - lo) / sr)

        print(f"{i+1:<3d}  {attack_s:>6.3f}s  {jump:>+6.1f}  {full_hole:>+7.1f} dB  {hf_hole:>+5.1f} dB  {mid_hole:>+6.1f} dB")
        hole_summary.append({
            'time_s': attack_s, 'jump_db': jump,
            'full_hole_db': full_hole,
            'hf_hole_db': hf_hole,
            'mid_hole_db': mid_hole,
        })

        # Plot: waveform (left), envelope overlay (right)
        ax = axes[i][0]
        ax.plot(t_seg, ref_seg, color='#8b94a5', lw=0.6, label='orig')
        ax.plot(t_seg, enc_seg, color='#3fb950', lw=0.6, alpha=0.7, label=label)
        ax.axvline(0, color='#f85149', lw=0.5, alpha=0.5)
        ax.set_title(f'Onset {i+1}  @ {attack_s:.3f}s   jump={jump:+.1f} dB')
        ax.set_ylabel('amplitude')
        ax.set_xlim(-args.window_ms, args.window_ms)
        ax.legend(loc='upper right', fontsize=8)
        ax.grid(True, alpha=0.2)

        ax = axes[i][1]
        ax.plot(t_env_ms, ref_env, color='#8b94a5', lw=1.2, label='orig full-band')
        ax.plot(t_env_ms, enc_env, color='#3fb950', lw=1.2, label=f'{label} full-band')
        ax.plot(t_env_ms, ref_hf_env, color='#d29922', lw=0.8, linestyle=':', label='orig 4-12 kHz')
        ax.plot(t_env_ms, enc_hf_env, color='#f85149', lw=0.8, linestyle=':', label=f'{label} 4-12 kHz')
        ax.axvline(0, color='#f85149', lw=0.5, alpha=0.5)
        ax.set_title(f'RMS envelope — hole depths: full {full_hole:+.1f}, HF {hf_hole:+.1f}, Mid {mid_hole:+.1f} dB')
        ax.set_xlabel('time relative to attack (ms)')
        ax.set_ylabel('RMS (dB)')
        ax.set_xlim(-args.window_ms, args.window_ms)
        ax.legend(loc='lower right', fontsize=7)
        ax.grid(True, alpha=0.2)

    fig.tight_layout()
    out = args.out or f'_tmp_at3stats/{Path(args.encoded).stem}_transient_zoom.png'
    Path(out).parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out, dpi=110)
    plt.close(fig)
    print(f"\nwrote {out}")

    # Aggregate judgment
    worst = max(hole_summary, key=lambda h: -h['full_hole_db'])
    print(f"\nworst full-band hole: {worst['full_hole_db']:+.1f} dB @ {worst['time_s']:.2f}s")
    hf_worst = max(hole_summary, key=lambda h: -h['hf_hole_db'])
    print(f"worst HF hole:        {hf_worst['hf_hole_db']:+.1f} dB @ {hf_worst['time_s']:.2f}s")
    if worst['full_hole_db'] < -3:
        print("→ AUDIBLE full-band hole around one or more transients")
    if hf_worst['hf_hole_db'] < -6:
        print("→ AUDIBLE HF hole — sibilance/sizzle missing near transients")


if __name__ == '__main__':
    main()
