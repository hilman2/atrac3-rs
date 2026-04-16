#!/usr/bin/env python
"""Digitale Ohren: vollständige perceptuelle + spektrale Analyse

Läuft standardmäßig auch die Artefakt-Lokalisation aus `artifact_finder.py`
am Ende der Messung — damit Pre-Echo-Onsets, HF-Bursts und Band-Rausch-
Verteilung bei JEDEM Testlauf mit angezeigt werden. So fallen hörbare
Kratz-/Zisch-Artefakte nicht erst beim Abhören auf.

Zum Deaktivieren der Artefakt-Sektion: `DIGITAL_EARS_NO_ARTIFACTS=1`.
"""
import os
import sys
# Force UTF-8 output so German umlauts and arrows render correctly on
# Windows consoles that default to cp1252.
try:
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
except Exception:
    pass
import numpy as np
from scipy.io import wavfile
from scipy import signal as sig

try:
    from artifact_finder import (
        find_onsets as _af_find_onsets,
        preecho_per_onset as _af_preecho,
        hf_burst_at_onsets as _af_hf_bursts,
        band_noise_ratio as _af_band_ratio,
        click_spike_finder as _af_click_spikes,
    )
    _ARTIFACTS_AVAILABLE = True
except Exception as _af_exc:
    _ARTIFACTS_AVAILABLE = False
    _ARTIFACTS_ERR = _af_exc

def sanity_check_stereo(r_raw, t_raw, label):
    """Front-loaded hard failure detection. Catches silent channels,
    clipping, and DC offsets BEFORE any perceptual metric can mask them."""
    alerts = []

    def ch_stats(x, name):
        out = {}
        if x.ndim == 1:
            out['mono'] = True
            x2d = x[:, None]
            ch_names = [name]
        else:
            out['mono'] = False
            x2d = x
            ch_names = [f"{name}[L]", f"{name}[R]"]
        for i, n in enumerate(ch_names):
            ch = x2d[:, i].astype(np.float64)
            rms = np.sqrt(np.mean(ch ** 2))
            peak = float(np.max(np.abs(ch)))
            dc = float(np.mean(ch))
            maxval = 32767.0 if ch.dtype.kind == 'i' else 1.0
            # clip detection: samples at the int16 rail
            clip_rate = float(np.mean(np.abs(ch) >= maxval - 1))
            out[n] = dict(rms=rms, peak=peak, dc=dc, clip_rate=clip_rate)
        return out

    r_s = ch_stats(r_raw, 'ref')
    t_s = ch_stats(t_raw, 'enc')

    # Stereo balance check on the encoded signal.
    if not t_s['mono']:
        lr = t_s['enc[L]']['rms']
        rr = t_s['enc[R]']['rms']
        if min(lr, rr) < 1.0:
            quieter = 'R' if lr > rr else 'L'
            alerts.append(
                f"CHANNEL DEAD: encoded {quieter} rms={min(lr, rr):.1f} (other={max(lr, rr):.1f}) — one channel is silent"
            )
        elif max(lr, rr) / max(min(lr, rr), 1e-9) > 2.0:
            db = 20 * np.log10(max(lr, rr) / max(min(lr, rr), 1e-9))
            quieter = 'R' if lr > rr else 'L'
            alerts.append(
                f"CHANNEL IMBALANCE: encoded {quieter} is {db:.1f} dB quieter than other side — likely encoder bug"
            )
        # Compare to reference imbalance (allow for originally asymmetric mixes).
        if not r_s['mono']:
            ref_ratio = r_s['ref[L]']['rms'] / max(r_s['ref[R]']['rms'], 1e-9)
            enc_ratio = lr / max(rr, 1e-9)
            drift_db = abs(20 * np.log10(max(enc_ratio / max(ref_ratio, 1e-9), 1e-9)))
            if drift_db > 3.0:
                alerts.append(
                    f"L/R BALANCE DRIFT: encoder shifted L/R ratio by {drift_db:.1f} dB vs reference"
                )

    # Clipping check — only fire when the encoder INTRODUCES clipping that
    # was not already present in the reference (Loudness-War masters clip
    # at int16 rails on purpose, so a raw threshold would false-alarm).
    def clip_excess(enc_key, ref_key):
        enc_clip = t_s[enc_key]['clip_rate']
        ref_clip = r_s[ref_key]['clip_rate'] if ref_key in r_s else 0.0
        excess = enc_clip - ref_clip
        if excess > 0.01:  # more than 1 percentage point above reference
            alerts.append(
                f"CLIPPING: {enc_key} has {enc_clip*100:.2f}% (ref {ref_clip*100:.2f}%) — encoder adds {excess*100:.2f}pp"
            )
    if not t_s['mono'] and not r_s['mono']:
        clip_excess('enc[L]', 'ref[L]')
        clip_excess('enc[R]', 'ref[R]')
    elif not t_s['mono']:
        clip_excess('enc[L]', 'ref')
        clip_excess('enc[R]', 'ref')
    else:
        clip_excess('enc', 'ref' if r_s['mono'] else 'ref[L]')

    # DC offset check
    def check_dc(stats, key, limit_rel=0.05):
        dc = stats[key]['dc']
        rms = max(stats[key]['rms'], 1.0)
        if abs(dc) > rms * limit_rel and abs(dc) > 50:
            alerts.append(f"DC OFFSET: {key} dc={dc:.1f} rms={rms:.1f} (dc/rms={abs(dc)/rms:.3f})")
    if not t_s['mono']:
        check_dc(t_s, 'enc[L]')
        check_dc(t_s, 'enc[R]')
    else:
        check_dc(t_s, 'enc')

    if alerts:
        print(f"\n{'!'*70}")
        print(f"  SANITY ALERTS for {label}")
        print('!'*70)
        for a in alerts:
            print(f"  [!!] {a}")
        # Show per-channel numbers so the user can confirm
        for name, stats in [('ref', r_s), ('enc', t_s)]:
            if stats['mono']:
                s = stats[name] if not name == 'ref' else stats['ref']
                print(f"  {name:10s}: rms={s['rms']:>8.1f} peak={s['peak']:>7.0f} clip%={s['clip_rate']*100:.2f}")
            else:
                for ch in [f'{name}[L]', f'{name}[R]']:
                    s = stats[ch]
                    print(f"  {ch:10s}: rms={s['rms']:>8.1f} peak={s['peak']:>7.0f} clip%={s['clip_rate']*100:.2f}")
        print('!'*70)
    return alerts


def write_spectrogram_diff(ref_path, test_path, out_path, duration_s=10):
    """Writes a 3-panel spectrogram PNG: original / encoded / error.
    The error panel reveals single-frame burst artifacts as bright vertical
    streaks at times unrelated to musical transients. Runs on every
    digital_ears.py call so regressions in frame-to-frame stability show
    up in CI-style comparisons."""
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt

    def load(path):
        sr, x = wavfile.read(path)
        x = x.astype(np.float32) / 32768.0
        if x.ndim > 1:
            x = x.mean(axis=1)
        return sr, x[:int(duration_s * sr)]

    sr, ref = load(ref_path)
    _, enc = load(test_path)
    n = min(len(ref), len(enc))
    ref = ref[:n]
    enc = enc[:n]
    noise = ref - enc

    f_r, t_r, s_r = sig.spectrogram(ref, sr, nperseg=1024, noverlap=896)
    f_t, t_t, s_t = sig.spectrogram(enc, sr, nperseg=1024, noverlap=896)
    f_n, t_n, s_n = sig.spectrogram(noise, sr, nperseg=1024, noverlap=896)
    eps = 1e-12

    fig, axes = plt.subplots(3, 1, figsize=(16, 11), sharex=True)
    im0 = axes[0].pcolormesh(t_r, f_r, 10 * np.log10(s_r + eps),
                             cmap='magma', vmin=-80, vmax=-10, shading='auto')
    axes[0].set_ylabel('Frequenz [Hz]')
    axes[0].set_title(f'Original (erste {duration_s:.0f}s)')
    axes[0].set_yscale('log'); axes[0].set_ylim(50, 20000)
    fig.colorbar(im0, ax=axes[0], label='dB')
    im1 = axes[1].pcolormesh(t_t, f_t, 10 * np.log10(s_t + eps),
                             cmap='magma', vmin=-80, vmax=-10, shading='auto')
    axes[1].set_ylabel('Frequenz [Hz]')
    axes[1].set_title('Encoded')
    axes[1].set_yscale('log'); axes[1].set_ylim(50, 20000)
    fig.colorbar(im1, ax=axes[1], label='dB')
    im2 = axes[2].pcolormesh(t_n, f_n, 10 * np.log10(s_n + eps),
                             cmap='hot', vmin=-80, vmax=-20, shading='auto')
    axes[2].set_ylabel('Frequenz [Hz]')
    axes[2].set_xlabel('Zeit [s]')
    axes[2].set_title('Fehler-Spektrum (ref - enc) — vertikale Streifen = Frame-Burst-Artefakte')
    axes[2].set_yscale('log'); axes[2].set_ylim(50, 20000)
    fig.colorbar(im2, ax=axes[2], label='Fehler [dB]')
    fig.tight_layout()
    fig.savefig(out_path, dpi=120)
    plt.close(fig)


def measure(ref_path, test_path, label):
    sr, r_raw = wavfile.read(ref_path)
    _, t_raw = wavfile.read(test_path)

    # Hard sanity checks first — these catch dead channels / clipping / DC
    # drift that downstream mono SNR would silently average away.
    sanity_check_stereo(r_raw, t_raw, label)

    r = r_raw.astype(np.float32)/32768.0; t = t_raw.astype(np.float32)/32768.0
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

    if _ARTIFACTS_AVAILABLE and not os.environ.get('DIGITAL_EARS_NO_ARTIFACTS'):
        # Re-load the full-length mono mix for artifact analysis (measure()
        # trimmed to channel 0 with a 2048-sample skip which is the right
        # choice for SNR but we want the mono downmix for onset detection
        # so all channels' transients count).
        r_mono = r_raw.astype(np.float32) / 32768.0
        t_mono = t_raw.astype(np.float32) / 32768.0
        if r_mono.ndim > 1:
            r_mono = r_mono.mean(axis=1)
        if t_mono.ndim > 1:
            t_mono = t_mono.mean(axis=1)
        n2 = min(len(r_mono), len(t_mono))
        r_mono = r_mono[2048:n2]
        t_mono = t_mono[2048:n2]

        print('-' * 70)
        print('Artefakt-Lokalisation:')
        rows = _af_band_ratio(r_mono, t_mono, sr)
        print(f"  {'Band':<12s}  {'SNR':>7s}  {'noise%':>7s}  {'bias':>7s}")
        worst_band = max(rows, key=lambda r: r[4])
        for name, snr, _ref_pct, noise_pct, bias in rows:
            marker = " <<<" if bias > 6 else ""
            print(f"  {name:<12s}  {snr:>6.1f}dB  {noise_pct:>6.1f}%  {bias:>+6.1f}dB{marker}")
        print(f"  -> Schlimmstes Band (relativer Rausch-Überschuss): {worst_band[0]} ({worst_band[4]:+.1f} dB über Signal-Share)")

        onsets = _af_find_onsets(r_mono, sr)
        if onsets:
            pe_list = _af_preecho(r_mono, t_mono, sr, onsets)
            if pe_list:
                pe_sorted = sorted(pe_list, key=lambda r: r['pe_db'], reverse=True)
                bad = [r for r in pe_list if r['pe_db'] > 3]
                print(f"  Transient-Events: {len(pe_list)}  mit Pre-Echo > 3 dB: {len(bad)} ({100*len(bad)/len(pe_list):.0f}%)")
                if pe_sorted[:5]:
                    worst_times = ', '.join(f"{r['time_s']:.2f}s({r['pe_db']:+.1f}dB)" for r in pe_sorted[:5])
                    print(f"  Worst pre-echo (Zeit / pe_db): {worst_times}")
            bursts = _af_hf_bursts(r_mono, t_mono, sr, onsets)
            print(f"  HF-Bursts vor Onsets (Kratz-/Zisch-Artefakt-Signal): {len(bursts)}")
            if bursts:
                b_sorted = sorted(bursts, key=lambda b: b['excess_db'], reverse=True)[:3]
                worst = ', '.join(f"{b['time_s']:.2f}s({b['excess_db']:+.1f}dB)" for b in b_sorted)
                print(f"  Worst HF-bursts: {worst}")

        clicks = _af_click_spikes(r_mono, t_mono, sr)
        print(f"  Kurze (30-200ms) Rausch-/Kratz-Bursts in ruhigen Passagen: {len(clicks)}")
        if clicks:
            print(f"    {'Zeit':>8s}  {'Dauer':>6s}  {'noise':>7s}  {'ref_quiet':>9s}  {'z':>5s}  {'peak_hz':>7s}")
            for c in clicks[:12]:
                print(f"    {c['time_s']:>7.3f}s  {c['duration_ms']:>5.1f}ms  {c['noise_db']:>+6.1f}dB  {c['ref_quiet_db']:>+8.1f}dB  {c['zscore']:>4.1f}  {c['band_peak_hz']:>6.0f}")
    elif not _ARTIFACTS_AVAILABLE:
        print('-' * 70)
        print(f"  (Artefakt-Sektion nicht verfügbar: {_ARTIFACTS_ERR})")

    if not os.environ.get('DIGITAL_EARS_NO_PLOT'):
        try:
            out_png = os.path.splitext(test_path)[0] + '_spectrogram.png'
            write_spectrogram_diff(ref_path, test_path, out_png, duration_s=10)
            print('-' * 70)
            print(f"Spektrogramm-Diff (erste 10s): {out_png}")
            print("  Sichtprüfung empfohlen — vertikale Streifen im Fehler-Spektrum (unteres")
            print("  Panel) sind Frame-Burst-Artefakte (Kratzen/Rauschen nicht an Musik gebunden).")
        except Exception as exc:
            print(f"  (Spektrogramm-Plot fehlgeschlagen: {exc})")

    return {
        'snr': overall, 'nf': avg_nf, 'pe_worst': pe_worst, 'hf_corr': hf_corr,
        'sf': sf_diff, 'phase': phase_err, 'bands': band_data,
    }

def compare_triple(original_path, sony_path, ours_path, label):
    """Three-way comparison:
       (1) ours vs original — how close we are to the perfect source
       (2) sony vs original — the ATRAC3-132k ceiling achievable at all
       (3) ours vs sony    — how close we match Sony's encoder

    Shows where the gap is: against ATRAC3-inherent loss vs Sony-specific
    algorithm choices."""
    print(f"\n{'#'*72}")
    print(f"  3-Fach Vergleich: {label}")
    print('#'*72)

    print(f"\n>>> (1) OURS  vs  ORIGINAL  (wie nah an der Source)")
    r1 = measure(original_path, ours_path, f"{label} | ours-vs-original")

    print(f"\n>>> (2) SONY  vs  ORIGINAL  (ATRAC3-132k-Grenze = erreichbares Maximum)")
    r2 = measure(original_path, sony_path, f"sony11 reference | sony-vs-original")

    print(f"\n>>> (3) OURS  vs  SONY  (bit-exakter Abgleich zum Sony-Encoder)")
    r3 = measure(sony_path, ours_path, f"{label} | ours-vs-sony")

    print(f"\n{'#'*72}")
    print(f"  ZUSAMMENFASSUNG 3-Fach ({label}):")
    print('#'*72)
    print(f"  SNR    — ours/original={r1['snr']:+6.2f}dB   sony/original={r2['snr']:+6.2f}dB   "
          f"gap-to-sony={r1['snr']-r2['snr']:+.2f}dB")
    print(f"  HFCorr — ours/original={r1['hf_corr']:+.3f}   sony/original={r2['hf_corr']:+.3f}")
    print(f"  OursvsSony SNR={r3['snr']:+.2f}dB  HFCorr={r3['hf_corr']:+.3f}")
    print(f"  (Ours-vs-Sony SNR nahe 0 = wir matchen Sony; hoher Wert = wir weichen stark ab)")


if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: digital_ears.py <test.wav> [label] [ref.wav]")
        print("       digital_ears.py --triple <original.wav> <sony.wav> <ours.wav> [label]")
        sys.exit(1)
    if sys.argv[1] == '--triple':
        if len(sys.argv) < 5:
            print("Triple usage: digital_ears.py --triple <original> <sony> <ours> [label]")
            sys.exit(1)
        original = sys.argv[2]
        sony = sys.argv[3]
        ours = sys.argv[4]
        label = sys.argv[5] if len(sys.argv) > 5 else ours
        compare_triple(original, sony, ours, label)
    else:
        test = sys.argv[1]
        label = sys.argv[2] if len(sys.argv) > 2 else test
        ref = sys.argv[3] if len(sys.argv) > 3 else 'testsong/HateMe.wav'
        measure(ref, test, label)
