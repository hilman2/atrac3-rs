#!/usr/bin/env python
"""hf_scope.py — finds and visualises HF artefacts that can't be seen
in the aggregate per-octave energy numbers.

An encoder that is within 0.1 dB of the reference on broadband HF RMS
can still emit audible artefacts: pumping (frame-to-frame level jumps),
noise bursts before/after transients, and spectral holes in sibilance.
This tool surfaces those by plotting *time-resolved* HF error metrics.

Usage:
  python hf_scope.py <original> <encoded> [--label frank]
     [--out hf_scope.png] [--hf-cutoff 4000]

Panels:
  1. HF-band error RMS over time (original − encoded) — spikes show
     frame-by-frame noise bursts.
  2. HF-band ENERGY envelope of original vs encoded, overlaid — any
     divergence (especially negative dips in encoded) means the encoder
     lost HF content at that moment.
  3. Short-time spectral-flatness delta (encoded − original) in the
     HF range — positive = encoded is rougher (bursty), negative =
     encoded is flatter than reference.
  4. Cumulative transient HF-noise: for each onset, how much extra HF
     noise sits in the 50 ms BEFORE the transient (pre-echo proxy).

Terminal output: aggregated artefact counts (bursts, pumping events,
HF holes) with timestamps so you can jump to them in the wav.
"""
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
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt


def load_mono(path):
    sr, x = wavfile.read(str(path))
    x = x.astype(np.float32) / 32768.0
    if x.ndim > 1:
        x = x.mean(axis=1)
    return sr, x


def hf_bandpass(x, sr, lo, hi):
    nyq = sr / 2
    hi = min(hi, nyq - 1)
    sos = sig.butter(6, [lo/nyq, hi/nyq], btype='band', output='sos')
    return sig.sosfilt(sos, x)


def framewise_rms_db(x, hop_s, win_s, sr):
    hop = int(hop_s * sr)
    win = int(win_s * sr)
    n_frames = max(1, (len(x) - win) // hop)
    out = np.zeros(n_frames)
    t = np.arange(n_frames) * hop_s
    for i in range(n_frames):
        seg = x[i*hop:i*hop+win]
        out[i] = 20 * np.log10(np.sqrt(np.mean(seg**2) + 1e-20))
    return t, out


def framewise_spectral_flatness(x, sr, hop_s, win_s, lo, hi):
    hop = int(hop_s * sr)
    win = int(win_s * sr)
    freqs = np.fft.rfftfreq(win, 1.0 / sr)
    mask = (freqs >= lo) & (freqs <= hi)
    n_frames = max(1, (len(x) - win) // hop)
    w = np.hanning(win)
    out = np.zeros(n_frames)
    for i in range(n_frames):
        seg = x[i*hop:i*hop+win] * w
        S = np.abs(np.fft.rfft(seg)) ** 2 + 1e-20
        band = S[mask]
        gm = np.exp(np.mean(np.log(band)))
        am = np.mean(band)
        out[i] = gm / am
    return out


def find_onsets(x, sr, threshold_db=4.0, min_gap_s=0.1):
    hop = 512
    win = 2048
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


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('original')
    ap.add_argument('encoded')
    ap.add_argument('--label', default=None)
    ap.add_argument('--out', default='_tmp_at3stats/hf_scope.png')
    ap.add_argument('--hf-cutoff', type=float, default=4000.0,
                    help='HF band lower edge in Hz (default 4000)')
    ap.add_argument('--hf-top', type=float, default=16000.0)
    ap.add_argument('--json', metavar='FILE',
                    help='write a machine-readable results dict to FILE')
    ap.add_argument('--quiet', action='store_true',
                    help='suppress the terminal report (pairs with --json)')
    ap.add_argument('--no-plot', action='store_true')
    args = ap.parse_args()

    label = args.label or Path(args.encoded).stem
    sr, ref = load_mono(args.original)
    _,  enc = load_mono(args.encoded)
    n = min(len(ref), len(enc))
    ref = ref[:n]; enc = enc[:n]

    hf_lo = args.hf_cutoff
    hf_hi = args.hf_top
    ref_hf = hf_bandpass(ref, sr, hf_lo, hf_hi)
    enc_hf = hf_bandpass(enc, sr, hf_lo, hf_hi)
    err_hf = ref_hf - enc_hf

    hop_s, win_s = 0.02, 0.05
    t_err, err_db = framewise_rms_db(err_hf, hop_s, win_s, sr)
    t_ref, ref_db = framewise_rms_db(ref_hf, hop_s, win_s, sr)
    _,     enc_db = framewise_rms_db(enc_hf, hop_s, win_s, sr)

    sfm_ref = framewise_spectral_flatness(ref, sr, hop_s, win_s, hf_lo, hf_hi)
    sfm_enc = framewise_spectral_flatness(enc, sr, hop_s, win_s, hf_lo, hf_hi)
    sfm_delta = sfm_enc[:len(sfm_ref)] - sfm_ref[:len(sfm_enc)]

    # Detect artefact events
    # 1. bursts: error RMS > threshold AND reference quiet at that time
    burst_thresh_db = -55.0          # error level
    quiet_ref_db = -40.0             # ref HF below this = quiet
    bursts = []
    for i in range(len(err_db)):
        if err_db[i] > burst_thresh_db and ref_db[i] < quiet_ref_db:
            bursts.append(t_err[i])
    # de-duplicate close-in-time events
    deduped = []
    for t in bursts:
        if not deduped or t - deduped[-1] > 0.2:
            deduped.append(t)
    bursts = deduped

    # 2. pumping: big jump in encoded HF RMS compared to reference jump
    pumping = []
    for i in range(1, len(enc_db)):
        ref_jump = abs(ref_db[i] - ref_db[i-1])
        enc_jump = abs(enc_db[i] - enc_db[i-1])
        if enc_jump > ref_jump + 8 and enc_jump > 5:
            pumping.append(t_err[i])

    # 3. HF holes: encoded HF far below reference HF
    holes = []
    for i in range(len(ref_db)):
        if ref_db[i] > -50 and enc_db[i] < ref_db[i] - 10:
            holes.append(t_err[i])
    deduped_holes = []
    for t in holes:
        if not deduped_holes or t - deduped_holes[-1] > 0.2:
            deduped_holes.append(t)
    holes = deduped_holes

    # 4. pre-echo around onsets: HF noise in 50 ms before onset
    onsets = find_onsets(ref, sr)
    pre_echo_events = []
    pre_ms, post_ms = 50, 50
    pre_n = int(pre_ms * sr / 1000); post_n = int(post_ms * sr / 1000)
    for o in onsets:
        if o - pre_n < 0 or o + post_n > len(ref):
            continue
        pre_err = err_hf[o-pre_n:o]
        post_err = err_hf[o:o+post_n]
        pre_rms = np.sqrt(np.mean(pre_err**2) + 1e-20)
        post_rms = np.sqrt(np.mean(post_err**2) + 1e-20)
        ratio_db = 20*np.log10(pre_rms / max(post_rms, 1e-12))
        if ratio_db > 3:
            pre_echo_events.append((o/sr, ratio_db, 20*np.log10(pre_rms)))

    # ---- compute aggregate metrics + artefact score ----
    hf_err_rms_db = float(20*np.log10(np.sqrt(np.mean(err_hf**2)) + 1e-20))
    hf_ref_rms_db = float(20*np.log10(np.sqrt(np.mean(ref_hf**2)) + 1e-20))
    hf_enc_rms_db = float(20*np.log10(np.sqrt(np.mean(enc_hf**2)) + 1e-20))
    hf_snr = float(10*np.log10(np.mean(ref_hf**2) / max(np.mean(err_hf**2), 1e-20)))

    duration_s = n / sr
    # normalise counts to events per 10 s so short-clip and long-clip
    # scores are comparable
    norm = 10.0 / max(duration_s, 0.1)

    bursts_per_10s = len(bursts) * norm
    pumping_per_10s = len(pumping) * norm
    holes_per_10s = len(holes) * norm
    pre_echo_events_per_10s = len(pre_echo_events) * norm

    # Aggregate artefact score: lower is better. Each term is in the
    # same "bad unit" scale (~1 per audible event).
    artefact_score = (
        1.0 * bursts_per_10s
      + 1.5 * pumping_per_10s
      + 2.0 * holes_per_10s
      + 0.5 * pre_echo_events_per_10s
    )
    # HF SNR deficit versus "transparent" (30 dB is Sony-ish on clean)
    hf_deficit = max(0.0, 30.0 - hf_snr)

    results = {
        'label': label,
        'original': str(args.original),
        'encoded': str(args.encoded),
        'sample_rate': int(sr),
        'duration_s': float(duration_s),
        'hf_band_hz': [float(hf_lo), float(hf_hi)],
        'hf_error_rms_dbfs': hf_err_rms_db,
        'hf_ref_rms_dbfs': hf_ref_rms_db,
        'hf_enc_rms_dbfs': hf_enc_rms_db,
        'hf_rms_delta_db': hf_enc_rms_db - hf_ref_rms_db,
        'hf_snr_db': hf_snr,
        'hf_snr_deficit_db': hf_deficit,
        'bursts': len(bursts),
        'bursts_per_10s': bursts_per_10s,
        'bursts_times_s': [float(t) for t in bursts[:16]],
        'pumping': len(pumping),
        'pumping_per_10s': pumping_per_10s,
        'pumping_times_s': [float(t) for t in pumping[:16]],
        'holes': len(holes),
        'holes_per_10s': holes_per_10s,
        'holes_times_s': [float(t) for t in holes[:16]],
        'pre_echo_events': len(pre_echo_events),
        'pre_echo_per_10s': pre_echo_events_per_10s,
        'pre_echo_worst_db': float(max((r for _, r, _ in pre_echo_events), default=0.0)),
        'artefact_score': float(artefact_score),
    }

    if not args.quiet:
        print(f"\n{'='*72}")
        print(f"hf_scope · {label} (band {hf_lo:.0f}-{hf_hi:.0f} Hz, {duration_s:.1f} s)")
        print('='*72)
        print(f"HF RMS   ref={hf_ref_rms_db:+.2f}   enc={hf_enc_rms_db:+.2f}   Δ={hf_enc_rms_db-hf_ref_rms_db:+.2f} dB")
        print(f"HF SNR   {hf_snr:+.2f} dB   (deficit vs 30 dB target: {hf_deficit:.2f})")
        print()
        print(f"Bursts/10s: {bursts_per_10s:>5.2f}   ({len(bursts)} total)")
        if bursts[:6]:
            print(f"  times: {', '.join(f'{t:.2f}s' for t in bursts[:6])}")
        print(f"Pumping/10s: {pumping_per_10s:>4.2f}   ({len(pumping)} total)")
        if pumping[:6]:
            print(f"  times: {', '.join(f'{t:.2f}s' for t in pumping[:6])}")
        print(f"Holes/10s:   {holes_per_10s:>4.2f}   ({len(holes)} total)")
        if holes[:6]:
            print(f"  times: {', '.join(f'{t:.2f}s' for t in holes[:6])}")
        print(f"Pre-echo/10s:{pre_echo_events_per_10s:>4.2f}   ({len(pre_echo_events)} total, worst={results['pre_echo_worst_db']:+.1f} dB)")
        print()
        print(f"ARTEFACT SCORE: {artefact_score:.2f}   (lower = cleaner; target < 2.0)")

    # ---- plot ----
    fig, axes = plt.subplots(4, 1, figsize=(16, 11), sharex=True)

    ax = axes[0]
    ax.plot(t_err, err_db, color='#f85149', lw=0.7, label='error RMS')
    ax.axhline(burst_thresh_db, color='#d29922', linestyle='--', lw=0.6,
               label=f'burst threshold {burst_thresh_db} dB')
    for t in bursts:
        ax.axvline(t, color='#d29922', alpha=0.2)
    ax.set_ylabel(f'Error RMS (dBFS)\nHF band {hf_lo:.0f}-{hf_hi:.0f} Hz')
    ax.set_title(f'HF noise over time — {label}')
    ax.legend(loc='upper right', fontsize=8)
    ax.grid(True, alpha=0.2)

    ax = axes[1]
    ax.plot(t_ref, ref_db, color='#8b94a5', lw=0.8, label='original')
    ax.plot(t_err, enc_db, color='#3fb950', lw=0.8, label=f'{label}')
    for t in holes:
        ax.axvline(t, color='#f85149', alpha=0.3)
    for t in pumping:
        ax.axvline(t, color='#58a6ff', alpha=0.3)
    ax.set_ylabel('HF band RMS (dBFS)')
    ax.set_title('HF envelope — red lines = HF holes, blue = pumping')
    ax.legend(loc='upper right', fontsize=8)
    ax.grid(True, alpha=0.2)

    ax = axes[2]
    ax.plot(t_err[:len(sfm_delta)], sfm_delta, color='#58a6ff', lw=0.7)
    ax.axhline(0, color='#444', lw=0.5)
    ax.set_ylabel('SFM delta\n(enc − ref)')
    ax.set_title('Spectral flatness delta (positive = encoded is rougher than ref)')
    ax.grid(True, alpha=0.2)

    ax = axes[3]
    ts = np.array([p[0] for p in pre_echo_events])
    rs = np.array([p[1] for p in pre_echo_events])
    if len(ts) > 0:
        ax.scatter(ts, rs, color='#d29922', s=18, alpha=0.7)
    ax.axhline(0, color='#444', lw=0.5)
    ax.axhline(3, color='#d29922', linestyle='--', lw=0.5, label='3 dB audible threshold')
    ax.set_xlabel('Time (s)')
    ax.set_ylabel('Pre/post HF-noise\nratio (dB)')
    ax.set_title('Per-onset pre-echo events (>3 dB = audible grit before transient)')
    ax.legend(loc='upper right', fontsize=8)
    ax.grid(True, alpha=0.2)

    fig.tight_layout()
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out, dpi=110)
    plt.close(fig)
    print(f"\nwrote {out}")


if __name__ == '__main__':
    main()
