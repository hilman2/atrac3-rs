#!/usr/bin/env python
"""Stereo-Ohren: Side-Signal-Energie, L-R-Correlation, stereo width per band"""
import sys
import numpy as np
from scipy.io import wavfile
from scipy import signal as sig

def load_stereo(p):
    sr, x = wavfile.read(p)
    x = x.astype(np.float32) / 32768.0
    if x.ndim == 1:
        x = np.stack([x, x], axis=1)
    return sr, x

def stereo_analysis(ref_path, test_path, label):
    sr, R = load_stereo(ref_path)
    _, T = load_stereo(test_path)
    n = min(len(R), len(T))
    R = R[2048:n]
    T = T[2048:n]

    # Mid/Side decomposition (common mastering tool)
    r_mid = (R[:, 0] + R[:, 1]) / 2
    r_side = (R[:, 0] - R[:, 1]) / 2
    t_mid = (T[:, 0] + T[:, 1]) / 2
    t_side = (T[:, 0] - T[:, 1]) / 2

    # Side energy ratio: how much stereo width is preserved
    # Side energy in dB relative to mid energy
    r_mid_e = np.mean(r_mid ** 2)
    r_side_e = np.mean(r_side ** 2)
    t_mid_e = np.mean(t_mid ** 2)
    t_side_e = np.mean(t_side ** 2)

    r_width = 10 * np.log10(r_side_e / max(r_mid_e, 1e-20))
    t_width = 10 * np.log10(t_side_e / max(t_mid_e, 1e-20))

    # Side signal SNR (how faithfully is the side signal reproduced)
    side_err = r_side - t_side
    side_snr = 10 * np.log10(r_side_e / max(np.mean(side_err ** 2), 1e-20))

    # Mid signal SNR
    mid_err = r_mid - t_mid
    mid_snr = 10 * np.log10(r_mid_e / max(np.mean(mid_err ** 2), 1e-20))

    # L-R correlation over time (how similar are L and R in each signal)
    def chunked_corr(a, b, chunk=4096):
        n = min(len(a), len(b))
        corrs = []
        for i in range(0, n - chunk, chunk):
            ca, cb = a[i:i+chunk], b[i:i+chunk]
            if np.std(ca) > 1e-6 and np.std(cb) > 1e-6:
                corrs.append(np.corrcoef(ca, cb)[0, 1])
        return np.mean(corrs) if corrs else 0

    r_lr_corr = chunked_corr(R[:, 0], R[:, 1])
    t_lr_corr = chunked_corr(T[:, 0], T[:, 1])

    # Phase coherence between channels (instantaneous phase diff spread)
    def phase_coherence(stereo):
        analytic_l = sig.hilbert(stereo[:, 0])
        analytic_r = sig.hilbert(stereo[:, 1])
        phase_diff = np.angle(analytic_l * np.conj(analytic_r))
        return np.std(phase_diff)

    r_phase_coh = phase_coherence(R)
    t_phase_coh = phase_coherence(T)

    # Per-band side energy comparison
    bands = [('Sub-Bass', 20, 80), ('Bass', 80, 250), ('Low-Mid', 250, 500),
             ('Mid', 500, 2000), ('Upper-Mid', 2000, 4000),
             ('Presence', 4000, 6000), ('Brilliance', 6000, 16000)]
    band_width = []
    for name, lo, hi in bands:
        nyq = sr / 2
        hi2 = min(hi, nyq - 1)
        sos = sig.butter(4, [lo/nyq, hi2/nyq], btype='band', output='sos')
        r_s = sig.sosfilt(sos, r_side)
        t_s = sig.sosfilt(sos, t_side)
        r_e = np.mean(r_s ** 2) + 1e-20
        t_e = np.mean(t_s ** 2) + 1e-20
        side_ratio_db = 10 * np.log10(t_e / r_e)
        snr = 10 * np.log10(r_e / max(np.mean((r_s - t_s) ** 2), 1e-20))
        band_width.append((name, side_ratio_db, snr))

    print(f'\n{"="*70}')
    print(f'  Stereo analysis: {label}')
    print('='*70)
    print(f'Mid/Side SNR:                 mid={mid_snr:.2f} dB   side={side_snr:.2f} dB')
    print(f'Side energy (width indicator):')
    print(f'  original:  {r_width:+.2f} dB vs mid')
    print(f'  encoded:   {t_width:+.2f} dB vs mid  (diff {t_width-r_width:+.2f})')
    print(f'L-R channel correlation:')
    print(f'  original:  {r_lr_corr:.4f}')
    print(f'  encoded:   {t_lr_corr:.4f}  (diff {t_lr_corr-r_lr_corr:+.4f})')
    print(f'L-R phase coherence spread (rad, higher = more stereo width):')
    print(f'  original:  {r_phase_coh:.4f}')
    print(f'  encoded:   {t_phase_coh:.4f}  (diff {t_phase_coh-r_phase_coh:+.4f})')
    print(f'\nPer-band Side-signal energy shift (negative = narrower than original):')
    print(f'{"Band":14s} | {"Side-energy shift (dB)":22s} | Side-SNR (dB)')
    for name, shift, snr in band_width:
        marker = '<<< too narrow' if shift < -0.3 else ('>>> too wide' if shift > 0.3 else 'ok')
        print(f'  {name:12s} | {shift:+.2f} {marker:18s} | {snr:.2f}')

    return {
        'r_width': r_width, 't_width': t_width,
        'r_lr_corr': r_lr_corr, 't_lr_corr': t_lr_corr,
        'side_snr': side_snr, 'mid_snr': mid_snr,
    }


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: stereo_ears.py <test.wav> [label] [ref.wav]")
        sys.exit(1)
    test = sys.argv[1]
    label = sys.argv[2] if len(sys.argv) > 2 else test
    ref = sys.argv[3] if len(sys.argv) > 3 else 'testsong/HateMe.wav'
    stereo_analysis(ref, test, label)
