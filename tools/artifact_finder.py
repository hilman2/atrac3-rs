#!/usr/bin/env python
"""Artefakt-Lokalisator: macht die Kratz-/Verwaschungs-Artefakte sichtbar
die man im Klangbild hört, als Zahlen und als PNG-Plot.

Kernidee: Der globale Pre-Echo-Wert aus digital_ears.py ist aggregiert.
Hier gehen wir pro Onset durch und zeigen:
  - die 20 schlimmsten Transient-Events mit Pre-Echo in dB
  - Spektrum des Quantisierungs-Rauschens gemittelt über 1 s Fenster
  - Zeitstempel der worst-offenders im Song (so dass man hinhören kann)
  - optionales Spektrogramm-Diff PNG
"""
import sys
import numpy as np
from scipy.io import wavfile
from scipy import signal as sig


def load_mono(path):
    sr, x = wavfile.read(path)
    x = x.astype(np.float32) / 32768.0
    if x.ndim > 1:
        x = x.mean(axis=1)  # mono mix preserves overall spectrum
    return sr, x


def find_onsets(signal, sr, threshold_db=6.0, min_gap_sec=0.12):
    """Energy-based onset detector. Returns list of sample indices."""
    hop = 512
    win = 2048
    frames = (len(signal) - win) // hop
    env = np.array([np.sqrt(np.mean(signal[i*hop:i*hop+win]**2)) for i in range(frames)])
    env_db = 20 * np.log10(env + 1e-10)
    onsets = []
    last = -10**9
    min_gap_frames = int(min_gap_sec * sr / hop)
    for i in range(1, len(env_db)):
        if env_db[i] - env_db[i-1] > threshold_db and env_db[i] > -40:
            if i - last > min_gap_frames:
                onsets.append(i * hop)
                last = i
    return onsets


def preecho_per_onset(ref, test, sr, onsets, pre_ms=30, post_ms=30):
    """For each onset, measure how much noise appears in the pre-region vs the
    post-region. If pre is much louder than post, that is audible "grit" in
    the moment before the transient — what the user described as 'kratzen'."""
    pre_n = int(pre_ms * sr / 1000)
    post_n = int(post_ms * sr / 1000)
    results = []
    for o in onsets:
        if o - pre_n < 0 or o + post_n >= len(ref):
            continue
        noise_pre = ref[o-pre_n:o] - test[o-pre_n:o]
        noise_post = ref[o:o+post_n] - test[o:o+post_n]
        ref_post = ref[o:o+post_n]
        if np.mean(ref_post**2) < 1e-10:
            continue
        pre_noise_db = 10 * np.log10(np.mean(noise_pre**2) + 1e-20)
        post_noise_db = 10 * np.log10(np.mean(noise_post**2) + 1e-20)
        # "Pre-echo dB" following the digital_ears.py convention:
        # pre-region noise relative to post-region noise (positive = bad).
        pe_db = pre_noise_db - post_noise_db
        # Absolute pre-region noise level relative to full-scale
        pre_rel_db = 10 * np.log10(np.mean(noise_pre**2) / max(np.mean(ref_post**2), 1e-20))
        results.append({
            'sample': o,
            'time_s': o / sr,
            'pe_db': pe_db,
            'pre_noise_db': pre_noise_db,
            'pre_rel_db': pre_rel_db,
        })
    return results


def noise_spectrum(ref, test, sr, win_sec=1.0):
    """Compute the average spectrum of the quantization noise (ref - test).
    This tells us WHICH frequency bands the noise lives in — HF-concentrated
    noise sounds like 'zischen', LF-concentrated like 'wummern', broadband
    transient-aligned like 'kratzen'."""
    n = int(win_sec * sr)
    noise = ref - test
    if len(noise) < n * 4:
        return None
    # average magnitude spectrum over disjoint 1s windows
    win = np.hanning(n)
    specs = []
    for start in range(0, len(noise) - n, n):
        chunk = noise[start:start+n] * win
        spec = np.abs(np.fft.rfft(chunk))
        specs.append(spec)
    avg = np.mean(np.stack(specs), axis=0)
    freqs = np.fft.rfftfreq(n, 1/sr)
    return freqs, avg


def band_noise_ratio(ref, test, sr):
    """What fraction of the noise energy lives in each perceptual band?
    Compared against the fraction of REFERENCE energy in that band.
    If noise-share > signal-share in a band, that band is where the
    encoder hurts the ear most."""
    bands = [('Sub-Bass', 20, 80), ('Bass', 80, 250), ('Low-Mid', 250, 500),
             ('Mid', 500, 2000), ('Upper-Mid', 2000, 4000),
             ('Presence', 4000, 6000), ('Brilliance', 6000, min(16000, sr/2-1))]
    results = []
    for name, lo, hi in bands:
        nyq = sr / 2
        sos = sig.butter(4, [lo/nyq, hi/nyq], btype='band', output='sos')
        rb = sig.sosfilt(sos, ref)
        tb = sig.sosfilt(sos, test)
        noise = rb - tb
        ref_energy = np.mean(rb**2)
        noise_energy = np.mean(noise**2)
        # Share of total energy — higher noise_share vs ref_share means
        # this band is "over-noisy" relative to its loudness.
        results.append((name, ref_energy, noise_energy))
    total_ref = sum(r for _, r, _ in results)
    total_noise = sum(n for _, _, n in results)
    rows = []
    for name, re_, ne_ in results:
        ref_share = re_ / max(total_ref, 1e-20)
        noise_share = ne_ / max(total_noise, 1e-20)
        snr = 10 * np.log10(re_ / max(ne_, 1e-20)) if ne_ > 0 else 99
        bias = 10 * np.log10(max(noise_share, 1e-10) / max(ref_share, 1e-10))
        rows.append((name, snr, ref_share * 100, noise_share * 100, bias))
    return rows


def hf_burst_at_onsets(ref, test, sr, onsets, pre_ms=10, hf_cutoff=4000):
    """Measure short HF-energy bursts in the encoded signal that are NOT
    present in the original — a classic lossy-codec 'pre-spray' artifact."""
    pre_n = int(pre_ms * sr / 1000)
    nyq = sr / 2
    sos = sig.butter(6, hf_cutoff/nyq, btype='high', output='sos')
    r_hf = sig.sosfilt(sos, ref)
    t_hf = sig.sosfilt(sos, test)
    bursts = []
    for o in onsets:
        if o - pre_n < 0:
            continue
        r_e = np.mean(r_hf[o-pre_n:o]**2)
        t_e = np.mean(t_hf[o-pre_n:o]**2)
        if r_e < 1e-8 and t_e > 4 * max(r_e, 1e-10):
            # 6 dB spurious burst
            excess_db = 10 * np.log10(t_e / max(r_e, 1e-10))
            bursts.append({'sample': o, 'time_s': o / sr, 'excess_db': excess_db})
    return bursts


def click_spike_finder(ref, test, sr, window_ms=25, step_ms=10, zscore_thresh=4.0,
                       min_duration_ms=30, max_duration_ms=200, max_hits=50):
    """Locate short isolated noise bursts (50-100ms garble) that are NOT
    tied to musical events. A burst shows up as a run of windows with
    noise-energy zscore much higher than the running median while the
    reference signal itself is in a quiet passage — i.e. the encoder
    slipped in a short garbled stretch that has no counterpart in the
    reference transient structure.

    Returns a list of {time_s, duration_ms, noise_db, ref_quiet_db,
    zscore, band_peak_hz} sorted by severity (duration × excess)."""
    win = int(window_ms * sr / 1000)
    step = int(step_ms * sr / 1000)
    if len(ref) < win * 20:
        return []
    noise = ref - test
    # noise energy per sliding window
    n_wins = (len(noise) - win) // step
    noise_e = np.empty(n_wins)
    ref_e = np.empty(n_wins)
    for i in range(n_wins):
        s = i * step
        noise_e[i] = np.mean(noise[s:s+win] ** 2)
        ref_e[i] = np.mean(ref[s:s+win] ** 2)
    # log-domain robust baseline
    log_noise = 10 * np.log10(noise_e + 1e-20)
    median = np.median(log_noise)
    mad = np.median(np.abs(log_noise - median)) + 1e-6
    # modified zscore (robust to outliers)
    z = 0.6745 * (log_noise - median) / mad
    # Only flag windows whose reference signal is NOT itself a transient —
    # click artifacts that live in quiet passages are what the user hears.
    log_ref = 10 * np.log10(ref_e + 1e-20)
    ref_median = np.median(log_ref)
    quiet_mask = log_ref < ref_median + 6  # not an obvious musical onset window
    hits = []
    i = 0
    while i < n_wins:
        if z[i] > zscore_thresh and quiet_mask[i]:
            j = i
            while j < n_wins and z[j] > zscore_thresh - 1.0 and quiet_mask[j]:
                j += 1
            dur_ms = ((j - i) * step) * 1000.0 / sr
            if dur_ms >= min_duration_ms and dur_ms <= max_duration_ms:
                s_start = i * step
                s_end = min(len(noise), j * step + win)
                chunk = noise[s_start:s_end]
                if len(chunk) >= 512:
                    spec = np.abs(np.fft.rfft(chunk * np.hanning(len(chunk))))
                    freqs = np.fft.rfftfreq(len(chunk), 1/sr)
                    peak_bin = int(np.argmax(spec))
                    band_peak_hz = float(freqs[peak_bin])
                else:
                    band_peak_hz = 0.0
                hits.append({
                    'time_s': (i * step) / sr,
                    'duration_ms': dur_ms,
                    'noise_db': float(log_noise[i:j].max()),
                    'ref_quiet_db': float(log_ref[i:j].max()),
                    'zscore': float(z[i:j].max()),
                    'band_peak_hz': band_peak_hz,
                })
            i = j + 1
        else:
            i += 1
    hits.sort(key=lambda h: -(h['zscore'] * h['duration_ms']))
    return hits[:max_hits]


def spectrogram_diff_plot(ref, test, sr, out_path):
    import matplotlib
    matplotlib.use('Agg')
    import matplotlib.pyplot as plt
    f_r, t_r, s_r = sig.spectrogram(ref, sr, nperseg=2048, noverlap=1536)
    f_t, t_t, s_t = sig.spectrogram(test, sr, nperseg=2048, noverlap=1536)
    # dB diff: positive = encoded has MORE energy than ref (added noise)
    #         negative = encoded has LESS energy than ref (lost content)
    eps = 1e-12
    diff_db = 10 * np.log10(s_t + eps) - 10 * np.log10(s_r + eps)
    fig, ax = plt.subplots(figsize=(14, 5))
    im = ax.pcolormesh(t_r, f_r, diff_db, cmap='RdBu_r', vmin=-20, vmax=20, shading='auto')
    ax.set_ylabel('Frequenz [Hz]')
    ax.set_xlabel('Zeit [s]')
    ax.set_yscale('log')
    ax.set_ylim(20, min(16000, sr/2))
    cbar = fig.colorbar(im, ax=ax)
    cbar.set_label('encoded - ref  [dB]  (rot=hinzugefügt, blau=verloren)')
    fig.tight_layout()
    fig.savefig(out_path, dpi=120)
    plt.close(fig)


def main():
    if len(sys.argv) < 3:
        print("Usage: artifact_finder.py <test.wav> <ref.wav> [--plot out.png]")
        sys.exit(1)
    test_path = sys.argv[1]
    ref_path = sys.argv[2]
    plot_path = None
    if '--plot' in sys.argv:
        i = sys.argv.index('--plot')
        plot_path = sys.argv[i + 1]

    sr_r, ref = load_mono(ref_path)
    sr_t, test = load_mono(test_path)
    assert sr_r == sr_t, f"sample rate mismatch: {sr_r} vs {sr_t}"
    sr = sr_r
    n = min(len(ref), len(test))
    # skip first 2048 samples — MDCT overlap causes deterministic start-up transient
    ref = ref[2048:n]
    test = test[2048:n]

    print(f"\n{'='*72}")
    print(f"  Artefakt-Lokalisator: {test_path}  vs  {ref_path}")
    print('='*72)
    duration = len(ref) / sr
    print(f"  Länge: {duration:.2f}s  @{sr}Hz")

    print("\n--- Per-Band Rausch-Anteil (wo lebt das Quantisierungs-Rauschen?) ---")
    print(f"  {'Band':<12s}  {'SNR':>7s}  {'ref%':>6s}  {'noise%':>6s}  {'bias':>7s}  Kommentar")
    for name, snr, ref_pct, noise_pct, bias in band_noise_ratio(ref, test, sr):
        comment = ""
        if bias > 6:
            comment = "<<< unverhältnismäßig laut verrauscht"
        elif bias < -6:
            comment = "ok — unterrepräsentiert im Rauschen"
        print(f"  {name:<12s}  {snr:>6.1f}dB  {ref_pct:>5.1f}%  {noise_pct:>5.1f}%  {bias:>+6.1f}dB  {comment}")

    print("\n--- Transient-Events (Pre-Echo pro Onset) ---")
    onsets = find_onsets(ref, sr)
    print(f"  Gefundene Onsets im Original: {len(onsets)}")
    if onsets:
        pe_results = preecho_per_onset(ref, test, sr, onsets)
        pe_results.sort(key=lambda r: r['pe_db'], reverse=True)
        print(f"  Schlimmste Pre-Echo-Events (pe_db = Rausch-Pegel_vor - Rausch-Pegel_nach):")
        print(f"  {'#':>3s}  {'Zeit[s]':>8s}  {'pe_db':>7s}  {'vor-Rausch':>11s}  {'vor/Signal':>11s}")
        for i, r in enumerate(pe_results[:15]):
            marker = "<<<" if r['pe_db'] > 3 else ""
            print(f"  {i+1:>3d}  {r['time_s']:>8.3f}  {r['pe_db']:>+6.1f}  {r['pre_noise_db']:>+10.1f}  {r['pre_rel_db']:>+10.1f}  {marker}")
        bad_count = sum(1 for r in pe_results if r['pe_db'] > 3)
        print(f"\n  Events mit Pre-Echo > 3dB: {bad_count}/{len(pe_results)} ({100*bad_count/max(len(pe_results),1):.1f}%)")

    print("\n--- HF-Bursts vor Onsets (typisches Kratz-/Zisch-Artefakt) ---")
    bursts = hf_burst_at_onsets(ref, test, sr, onsets)
    print(f"  Spurious HF-Bursts (encoded hat HF-Energie vor dem Onset die das Original nicht hat): {len(bursts)}")
    if bursts:
        bursts.sort(key=lambda b: b['excess_db'], reverse=True)
        print(f"  {'#':>3s}  {'Zeit[s]':>8s}  {'Überschuss-dB':>14s}")
        for i, b in enumerate(bursts[:10]):
            print(f"  {i+1:>3d}  {b['time_s']:>8.3f}  {b['excess_db']:>+13.1f}")

    print("\n--- Rausch-Spektrum (gemittelt, gesamt) ---")
    ns = noise_spectrum(ref, test, sr, win_sec=1.0)
    if ns is not None:
        freqs, spec = ns
        # Report octave band levels so it's readable
        octaves = [125, 250, 500, 1000, 2000, 4000, 8000, 16000]
        prev = 0
        print(f"  {'Oktave':>10s}  {'rms (rel)':>10s}")
        for f in octaves:
            if f > sr/2:
                break
            mask = (freqs >= prev) & (freqs < f)
            if np.any(mask):
                energy = np.sqrt(np.mean(spec[mask]**2))
                db = 20 * np.log10(energy + 1e-12)
                print(f"  {f:>8d}Hz  {db:>+9.1f}dB")
            prev = f

    if plot_path:
        spectrogram_diff_plot(ref, test, sr, plot_path)
        print(f"\n  Spektrogramm-Diff gespeichert: {plot_path}")


if __name__ == '__main__':
    main()
