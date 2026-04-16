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


def _find_transients_for_hole(x, sr, n_max=5, min_jump_db=2.0):
    """Top-N strongest energy-jump transients, for HF-hole measurement."""
    hop, win = 256, 1024
    frames = (len(x) - win) // hop
    if frames <= 0:
        return []
    env = np.array([np.sqrt(np.mean(x[i*hop:i*hop+win]**2)) for i in range(frames)])
    env_db = 20 * np.log10(env + 1e-10)
    cands = []
    for i in range(2, len(env_db)):
        jump = env_db[i] - env_db[i-1]
        if jump > min_jump_db and env_db[i] > -40:
            cands.append((i * hop, jump))
    merge = int(0.15 * sr)
    cands.sort(key=lambda c: -c[1])
    picked = []
    for pos, jump in cands:
        if all(abs(pos - p) > merge for p, _ in picked):
            picked.append((pos, jump))
        if len(picked) >= n_max:
            break
    picked.sort(key=lambda c: c[0])
    return [p for p, _ in picked]


def _envelope_db(x, sr, hop_ms=2.0, win_ms=5.0):
    hop = int(hop_ms * sr / 1000)
    win = int(win_ms * sr / 1000)
    n = max(1, (len(x) - win) // hop)
    out = np.zeros(n)
    for i in range(n):
        seg = x[i*hop:i*hop+win]
        out[i] = 20 * np.log10(np.sqrt(np.mean(seg**2)) + 1e-10)
    t = np.arange(n) * hop_ms / 1000.0
    return t, out


def _hole_depth(ref_env, enc_env, t, attack_s, guard_ms=5.0, window_ms=150.0):
    """Max dB drop of enc below ref in ±window_ms of attack, excluding
    ±guard_ms around the attack. Returns a non-positive number."""
    lo = attack_s - window_ms / 1000.0
    hi = attack_s + window_ms / 1000.0
    g_lo = attack_s - guard_ms / 1000.0
    g_hi = attack_s + guard_ms / 1000.0
    mask = ((t >= lo) & (t <= hi)) & ~((t >= g_lo) & (t <= g_hi))
    if not mask.any():
        return 0.0
    delta = enc_env[mask] - ref_env[mask]
    return float(delta.min())


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

# ---------- source health classification ----------

def source_health(x, sr):
    """Classify the reference itself so we don't punish the encoder for
    what the source is missing.

    Three independent indicators, combined into one class:

    1. hf_energy_db   — 14-22 kHz RMS vs full-band RMS, in dB.
                        Electronic genres naturally sit low here, so
                        alone this is genre-biased.

    2. sfm_hf         — spectral flatness of 4-16 kHz frames.
                        MP3-style quantiser noise is flat (SFM → 1).
                        Real HF with structure has peaks (SFM → 0).
                        A high SFM combined with low HF energy is a
                        strong MP3 fingerprint.

    3. cliff_slope_db — difference of band energy across the canonical
                        MP3 low-pass cliff (~15-17 kHz vs 18-20 kHz).
                        A sharp drop > 15 dB is the classic 128 kbps
                        MP3 brickwall.

    Decision: start with the HF-energy tier, then upgrade to severe if
    SFM is high or the cliff is steep (both indicate lossy upstream).
    Downgrade to clean if SFM is low (tonal HF) even when HF energy is
    modest — that rescues Electronic genres with natural low-ish air.
    """
    full_rms = np.sqrt(np.mean(x**2) + 1e-20)
    full_db = 20 * np.log10(full_rms)
    air = band_bandpass(x, sr, 14000, min(sr/2 - 1, 22000))
    air_db = 20 * np.log10(np.sqrt(np.mean(air**2) + 1e-20))
    hf_energy_db = air_db - full_db

    # SFM in 4-16 kHz
    win = 2048
    w = np.hanning(win)
    freqs = np.fft.rfftfreq(win, 1.0/sr)
    mask_hf = (freqs >= 4000) & (freqs <= min(16000, sr/2 - 1))
    sfm_vals = []
    for i in range(0, len(x) - win, win):
        S = np.abs(np.fft.rfft(x[i:i+win] * w)) ** 2 + 1e-20
        band = S[mask_hf]
        gm = np.exp(np.mean(np.log(band)))
        am = np.mean(band)
        sfm_vals.append(gm / am)
    sfm_hf = float(np.mean(sfm_vals)) if sfm_vals else 1.0

    # Cliff detection: 15-17 kHz vs 18-20 kHz RMS delta.
    # MP3 128 kbps has a ~15-17 kHz cutoff, so there's a steep fall
    # (> 15 dB) right there. Natural sources have a gentle roll-off.
    pre_cliff = band_bandpass(x, sr, 15000, 17000)
    post_cliff = band_bandpass(x, sr, 18000, min(20000, sr/2 - 1))
    pre_db = 20 * np.log10(np.sqrt(np.mean(pre_cliff**2) + 1e-20))
    post_db = 20 * np.log10(np.sqrt(np.mean(post_cliff**2) + 1e-20))
    cliff_slope_db = pre_db - post_db

    # Fusion rule, ordered most-to-least trusted. The cliff is the
    # smoking gun for a lossy upstream codec — no natural source rolls
    # off steeper than 25 dB inside a 3-kHz window. An absent cliff
    # means the HF energy level is a genre artefact, not an encoder
    # artefact, and the source should be treated as clean.
    if cliff_slope_db > 25.0:
        cls, tmult = 'lossy_severe', 2.0
    elif cliff_slope_db > 15.0 or hf_energy_db < -38.0:
        # Moderate cliff, or truly empty HF → lossy-mild band.
        cls, tmult = 'lossy_mild', 1.3
    elif cliff_slope_db < 12.0:
        # Natural roll-off ⇒ clean, irrespective of HF energy level
        # (Electronic/Classical naturally sit low in 14+ kHz).
        cls, tmult = 'clean', 1.0
    else:
        # Ambiguous; defer to the old HF+SFM combination.
        if hf_energy_db < -30.0 and sfm_hf > 0.25:
            cls, tmult = 'lossy_mild', 1.3
        else:
            cls, tmult = 'clean', 1.0

    return {
        'hf_energy_db': float(hf_energy_db),
        'spectral_flatness_hf': sfm_hf,
        'cliff_slope_db': float(cliff_slope_db),
        'class': cls,
        'tolerance_mult': tmult,
    }


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
    # Stereo-aware penalties (otherwise the mono downmix hides
    # interchannel-masking regressions).
    'side_snr_deficit_db':  (0.4, 'Side-signal SNR deficit (dB)', 0.0),
    'width_shift_db':       (2.0, 'Stereo width shift',    0.3),
    'lr_corr_shift':        (3.0, 'L-R correlation shift', 0.02),
    # Transient-neighbourhood HF hole — the audible "before/after the
    # snare is a gap" artefact. Sony sits at ~1.3 dB, Frank/Classic at
    # ~2.3-2.5, so a 1.5 dB tolerance catches the outliers without
    # penalising acceptable encoders.
    'snare_hf_hole_db':     (1.5, 'HF hole near transients (dB)', 1.5),
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

    # Classify the source first. Lossy sources get wider tolerance
    # bands because no encoder can resurrect HF the source doesn't
    # have. This keeps the score honest: an encoder handed a 128 kbps
    # MP3 re-encode isn't penalised for the upstream codec's sins.
    src = source_health(ref, sr_ref)
    tmult = src['tolerance_mult']

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

    # Stereo quality. A mono downmix blinds ear_judge to
    # interchannel-masking artefacts (an encoder can quietly throw
    # away one channel's detail without the mono SNR moving). Measure
    # the side signal directly and the stereo-width / L-R-correlation
    # drift — the user-reported audibility of a stereo change.
    side_snr_db = 30.0
    width_shift_db = 0.0
    lr_corr_shift = 0.0
    try:
        _, ref_st = load_stereo(ref_path)
        _, enc_st = load_stereo(enc_path)
        n2 = min(len(ref_st), len(enc_st))
        ref_st = ref_st[:n2]
        enc_st = enc_st[:n2]
        r_mid = (ref_st[:, 0] + ref_st[:, 1]) * 0.5
        r_side = (ref_st[:, 0] - ref_st[:, 1]) * 0.5
        t_mid = (enc_st[:, 0] + enc_st[:, 1]) * 0.5
        t_side = (enc_st[:, 0] - enc_st[:, 1]) * 0.5
        r_mid_e = float(np.mean(r_mid ** 2) + 1e-20)
        r_side_e = float(np.mean(r_side ** 2) + 1e-20)
        t_mid_e = float(np.mean(t_mid ** 2) + 1e-20)
        t_side_e = float(np.mean(t_side ** 2) + 1e-20)
        side_err_e = float(np.mean((r_side - t_side) ** 2) + 1e-20)
        # Side SNR: how faithfully is the side signal preserved.
        # Low = stereo detail damaged.
        side_snr_db = 10.0 * np.log10(r_side_e / side_err_e)
        r_width = 10.0 * np.log10(r_side_e / r_mid_e)
        t_width = 10.0 * np.log10(t_side_e / t_mid_e)
        width_shift_db = abs(t_width - r_width)
        def _corr(a, b):
            if np.std(a) < 1e-6 or np.std(b) < 1e-6:
                return 1.0
            return float(np.corrcoef(a, b)[0, 1])
        lr_corr_shift = abs(_corr(enc_st[:, 0], enc_st[:, 1])
                          - _corr(ref_st[:, 0], ref_st[:, 1]))
    except Exception:
        pass
    # Side-SNR deficit: 30 dB is "transparent-ish". Anything below
    # we count the shortfall as penalty.
    side_snr_deficit_db = max(0.0, 30.0 - side_snr_db)

    # HF hole around transient onsets. Measures how much quieter the
    # encoded 4-12 kHz band is compared to the reference in a ±150 ms
    # window around each transient attack, excluding a ±5 ms guard
    # around the attack itself. Classic/Frank drop 2-3 dB there on
    # snares; Sony drops only 1-1.5 dB. Average the top three onsets
    # so a single outlier doesn't drive the metric but a consistent
    # issue does.
    snare_hf_hole_db = 0.0
    try:
        onsets = _find_transients_for_hole(ref, sr_ref, n_max=5)
        holes = []
        for pos in onsets:
            half = int(0.2 * sr_ref)
            lo = max(0, pos - half); hi = min(len(ref), pos + half)
            ref_seg = ref[lo:hi]; enc_seg = enc[lo:hi]
            ref_hf = band_bandpass(ref_seg, sr_ref, 4000, min(sr_ref/2 - 1, 12000))
            enc_hf = band_bandpass(enc_seg, sr_ref, 4000, min(sr_ref/2 - 1, 12000))
            t_env, ref_hf_env = _envelope_db(ref_hf, sr_ref)
            _,     enc_hf_env = _envelope_db(enc_hf, sr_ref)
            h = _hole_depth(ref_hf_env, enc_hf_env, t_env, (pos - lo)/sr_ref)
            holes.append(h)
        if holes:
            holes.sort()  # most negative first
            top = holes[:3]
            # we report the penalty as |mean of top-3 worst| (positive)
            snare_hf_hole_db = abs(sum(top) / len(top))
    except Exception:
        pass

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
        'side_snr_deficit_db': side_snr_deficit_db,
        'width_shift_db':      width_shift_db,
        'lr_corr_shift':       lr_corr_shift,
        'snare_hf_hole_db':    snare_hf_hole_db,
    }
    breakdown = {}
    score = 0.0
    for key, (w, _, tol) in PEN_WEIGHTS.items():
        # Lossy sources get wider tolerance on the terms that the
        # upstream codec already damaged: HF artefact counts,
        # pre-echo, octave balance. Onset fidelity and loudness are
        # source-agnostic — we don't loosen those.
        tol_source_adjusted = (
            tol * tmult
            if key in ('hf_bursts_per_10s', 'hf_pumping_per_10s',
                       'hf_holes_per_10s', 'pre_echo_per_10s',
                       'pre_echo_worst_db', 'octave_balance',
                       'nmr_max_over', 'side_snr_deficit_db',
                       'snare_hf_hole_db')
            else tol
        )
        excess = max(0.0, terms[key] - tol_source_adjusted)
        contrib = w * excess
        breakdown[key] = {
            'value': terms[key],
            'tolerance': tol_source_adjusted,
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
        'source_class': src['class'],
        'source_hf_energy_db': src['hf_energy_db'],
        'source_sfm_hf': src['spectral_flatness_hf'],
        'source_cliff_slope_db': src['cliff_slope_db'],
        'source_tolerance_mult': src['tolerance_mult'],
        'score': float(score),
        'verdict': verdict,
        'breakdown': breakdown,
        'context': {
            'overall_snr_db': float(overall_snr),
            'hf_snr_db': hf['hf_snr_db'],
            'side_snr_db': float(side_snr_db),
            'width_shift_db': float(width_shift_db),
            'lr_corr_shift': float(lr_corr_shift),
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
    print(f"{'source':<20s}  {result['source_class']:<14s}  HF={result['source_hf_energy_db']:+.1f} dB  "
          f"SFM={result['source_sfm_hf']:.2f}  cliff={result['source_cliff_slope_db']:+.1f} dB  "
          f"tolerance×{result['source_tolerance_mult']:.1f}")
    print(f"{'context':<20s}  overall SNR {result['context']['overall_snr_db']:+.2f} dB · "
          f"HF SNR {result['context']['hf_snr_db']:+.2f} dB · "
          f"Side SNR {result['context']['side_snr_db']:+.2f} dB · "
          f"widthΔ {result['context']['width_shift_db']:+.2f}")
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
    # Source context in the comparison header.
    first = results_sorted[0]
    print(f"source: {first['source_class']:<12s}  HF={first['source_hf_energy_db']:+.1f} dB  "
          f"SFM={first['source_sfm_hf']:.2f}  cliff={first['source_cliff_slope_db']:+.1f}  "
          f"tolerance×{first['source_tolerance_mult']:.1f}")
    print()
    print(f"{'rank':<5s} {'label':<20s} {'score':>8s}  {'verdict':<14s} {'SNR':>7s} {'HF-SNR':>7s} "
          f"{'Side':>6s} {'widΔ':>5s} {'voc':>6s}  {'hf-art':>7s}")
    for rank, r in enumerate(results_sorted, 1):
        vc_shift = r['breakdown']['vocal_clarity_shift']['value']
        hf_art = (r['breakdown']['hf_bursts_per_10s']['contribution']
                + r['breakdown']['hf_pumping_per_10s']['contribution']
                + r['breakdown']['hf_holes_per_10s']['contribution'])
        print(f"{rank:<5d} {r['label']:<20s} {r['score']:>7.2f}   {r['verdict']:<14s} "
              f"{r['context']['overall_snr_db']:>+6.1f}  {r['context']['hf_snr_db']:>+6.1f}  "
              f"{r['context']['side_snr_db']:>+5.1f} {r['context']['width_shift_db']:>+4.2f} "
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
