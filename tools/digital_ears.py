#!/usr/bin/env python
"""Digitale Ohren: vollständige perceptuelle + spektrale Analyse"""
import sys
import numpy as np
from scipy.io import wavfile
from scipy import signal as sig

def measure(ref_path, test_path, label):
    sr, r = wavfile.read(ref_path)
    _, t = wavfile.read(test_path)
    r = r.astype(np.float32)/32768.0; t = t.astype(np.float32)/32768.0
    if r.ndim>1: r=r[:,0]
    if t.ndim>1: t=t[:,0]
    n=min(len(r),len(t)); r=r[2048:n]; t=t[2048:n]
    noise = r - t

    overall = 10*np.log10(np.mean(r**2)/max(np.mean(noise**2),1e-20))

    nf = []
    for i in range(0, len(r)-11025, 11025):
        if np.mean(r[i:i+11025]**2) < 1e-4:
            nl = np.mean(noise[i:i+11025]**2)
            if nl > 0: nf.append(10*np.log10(nl))
    avg_nf = np.mean(nf) if nf else -99

    fs = 1024
    energies = np.array([np.mean(r[i:i+fs]**2) for i in range(0, len(r)-fs, fs)])
    trans = [i*fs for i in range(1, len(energies)) if energies[i]>energies[i-1]*4 and energies[i]>1e-6]
    pe = []
    for a in trans[:30]:
        ps = max(0, a-fs*2); pe_end = a; pos = a; poe = min(len(r), a+fs*2)
        pr = np.mean((r[ps:pe_end]-t[ps:pe_end])**2)
        po = np.mean((r[pos:poe]-t[pos:poe])**2)
        if po > 0: pe.append(10*np.log10(pr/po))
    pe_worst = max(pe) if pe else -99
    pe_avg = np.mean(pe) if pe else -99

    sos_hf = sig.butter(4, 8000/(sr/2), btype='high', output='sos')
    r_env = np.abs(sig.hilbert(sig.sosfilt(sos_hf, r)))
    t_env = np.abs(sig.hilbert(sig.sosfilt(sos_hf, t)))
    hf_corr = np.corrcoef(r_env[::100], t_env[::100])[0,1]

    win = np.hanning(2048); flat = []
    for i in range(0, len(r)-2048, 1024):
        rs = np.abs(np.fft.rfft(r[i:i+2048]*win))+1e-10
        ts = np.abs(np.fft.rfft(t[i:i+2048]*win))+1e-10
        flat.append(np.exp(np.mean(np.log(ts)))/np.mean(ts) - np.exp(np.mean(np.log(rs)))/np.mean(rs))
    sf_diff = np.mean(flat)

    phase_errs = []
    for i in range(0, len(r)-2048, 2048):
        rs = np.fft.rfft(r[i:i+2048]*win); ts = np.fft.rfft(t[i:i+2048]*win)
        mag = np.abs(rs)*np.abs(ts)
        pd = np.angle(rs) - np.angle(ts)
        pd = np.arctan2(np.sin(pd), np.cos(pd))
        phase_errs.append(np.sum(mag*np.abs(pd))/max(np.sum(mag), 1e-10))
    phase_err = np.mean(phase_errs)

    bands = [('Sub-Bass',20,80),('Bass',80,250),('Low-Mid',250,500),
             ('Mid',500,2000),('Upper-Mid',2000,4000),
             ('Presence',4000,6000),('Brilliance',6000,16000)]
    band_data = {}
    for name, lo, hi in bands:
        nyq = sr/2; hi2 = min(hi, nyq-1)
        sos = sig.butter(4, [lo/nyq, hi2/nyq], btype='band', output='sos')
        rb = sig.sosfilt(sos, r); tb = sig.sosfilt(sos, t)
        snr = 10*np.log10(np.mean(rb**2)/max(np.mean((rb-tb)**2), 1e-20))
        en = 20*np.log10(max(np.sqrt(np.mean(tb**2)/max(np.mean(rb**2), 1e-20)), 1e-10))
        band_data[name] = (snr, en)

    sony_ref = {
        'snr': 20.4, 'nf': -81.0, 'pe_worst': 2.1, 'hf_corr': 0.928,
        'sf': -0.001, 'phase': 0.034,
        'Sub-Bass': 34.8, 'Bass': 26.0, 'Low-Mid': 23.3, 'Mid': 18.9,
        'Upper-Mid': 13.1, 'Presence': 9.8, 'Brilliance': 7.7,
    }

    print(f"\n{'='*70}")
    print(f"  {label}")
    print('='*70)
    print(f"{'Metric':28s} | {'Ours':>10s} | {'Sony':>10s} | Diff")
    print('-'*70)
    print(f"{'SNR (dB)':28s} | {overall:>10.2f} | {sony_ref['snr']:>10.1f} | {overall-sony_ref['snr']:+.2f}")
    print(f"{'Noise Floor (dBFS)':28s} | {avg_nf:>10.2f} | {sony_ref['nf']:>10.1f} | {avg_nf-sony_ref['nf']:+.2f}")
    print(f"{'Pre-Echo worst (dB)':28s} | {pe_worst:>10.2f} | {sony_ref['pe_worst']:>10.1f} | {pe_worst-sony_ref['pe_worst']:+.2f}  (lower better)")
    print(f"{'Pre-Echo avg (dB)':28s} | {pe_avg:>10.2f} |          - |")
    print(f"{'HF Envelope Corr':28s} | {hf_corr:>10.3f} | {sony_ref['hf_corr']:>10.3f} | {hf_corr-sony_ref['hf_corr']:+.3f}  (higher better)")
    print(f"{'|Spectral Flatness|':28s} | {abs(sf_diff):>10.4f} | {abs(sony_ref['sf']):>10.4f} |")
    print(f"{'Phase err (rad)':28s} | {phase_err:>10.3f} | {sony_ref['phase']:>10.3f} |")
    print('-'*70)
    print('Per-Band SNR (dB):')
    for name, (snr, en) in band_data.items():
        sony_v = sony_ref.get(name, 0)
        print(f"  {name:14s} | {snr:>10.2f} | {sony_v:>10.1f} | {snr-sony_v:+.2f}  E={en:+.2f}dB")

    return {
        'snr': overall, 'nf': avg_nf, 'pe_worst': pe_worst, 'hf_corr': hf_corr,
        'sf': sf_diff, 'phase': phase_err, 'bands': band_data,
    }

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: digital_ears.py <test.wav> [label] [ref.wav]")
        sys.exit(1)
    test = sys.argv[1]
    label = sys.argv[2] if len(sys.argv) > 2 else test
    ref = sys.argv[3] if len(sys.argv) > 3 else 'testsong/HateMe.wav'
    measure(ref, test, label)
