//! Psycho-Drive — single per-frame psychoacoustic analysis.
//!
//! Computes the masking curve, tonality, ATH floor, and perceptual
//! entropy in one pass over the MDCT coefficients. Every later stage
//! (allocator, gain, stereo) reads from this struct rather than
//! recomputing its own local heuristic.
//!
//! References:
//!  - Zwicker / Fastl: *Psychoacoustics — Facts and Models* (Bark bands,
//!    spreading function).
//!  - Terhardt 1979 (ATH formula).
//!  - ISO/IEC 11172-3 Annex D Psychoacoustic Model 2 (SFM-based
//!    tonality, perceptual entropy).

use std::cell::RefCell;

use crate::atrac3::{SAMPLES_PER_FRAME, quant::ATRAC3_SUBBAND_TAB};

thread_local! {
    /// Per-channel state. ATRAC3 encodes L and R into separate Sound
    /// Units per frame, called sequentially; a single thread_local slot
    /// would overwrite L's state when R is processed and vice versa.
    /// Indexed by channel (0 = L, 1 = R).
    static PREV_QMF_ENERGY: RefCell<[[f32; 4]; 2]> =
        const { RefCell::new([[0.0; 4]; 2]) };
    static PREV_TRANSIENT: RefCell<[[bool; 4]; 2]> =
        const { RefCell::new([[false; 4]; 2]) };
    static PREV_MAG: RefCell<[[f32; 32]; 2]> =
        const { RefCell::new([[0.0; 32]; 2]) };
    static PREV_PREV_MAG: RefCell<[[f32; 32]; 2]> =
        const { RefCell::new([[0.0; 32]; 2]) };
    /// Current-frame subband energies of the OTHER channel. Written by
    /// channel 0 (L) at the start of its compute(), read by channel 1
    /// (R) in the same frame. Enables interchannel masking: a loud L
    /// band masks the same-frequency R band perceptually (binaural
    /// summation across cochlear overlap), letting the encoder spend
    /// fewer bits on R's copy of centered content.
    static CROSS_CHAN_ENERGY: RefCell<[f32; 32]> =
        const { RefCell::new([-120.0; 32]) };
}

/// Number of Bark critical bands we use (Zwicker 1980).
pub const N_BARK: usize = 24;

/// Upper edge of each Bark band in Hz (band `i` covers edges[i]..edges[i+1]).
pub const BARK_EDGES_HZ: [f32; N_BARK + 1] = [
    0.0, 100.0, 200.0, 300.0, 400.0, 510.0, 630.0, 770.0, 920.0, 1080.0,
    1270.0, 1480.0, 1720.0, 2000.0, 2320.0, 2700.0, 3150.0, 3700.0,
    4400.0, 5300.0, 6400.0, 7700.0, 9500.0, 12000.0, 22050.0,
];

/// Output of the psychoacoustic analysis, consumed by every later
/// encoder stage.
#[derive(Debug, Clone)]
pub struct PsychoDrive {
    /// Signal energy in dB per Bark band (24 bands).
    pub bark_energy_db: [f32; N_BARK],
    /// Effective masking floor in dB projected back onto the 32 ATRAC3
    /// subbands. For each subband this is the highest of:
    ///   - own-band signal minus tonality-adjusted SMR
    ///   - spread masking from adjacent Bark bands
    ///   - absolute threshold of hearing (ATH)
    pub subband_mask_db: [f32; 32],
    /// Subband-level signal energy in dB for convenience (avoids
    /// repeated recomputation in the allocator).
    pub subband_energy_db: [f32; 32],
    /// Tonality in [0, 1] per subband. 0 = flat noise, 1 = pure tone.
    /// Derived from the spectral flatness measure (SFM).
    pub tonality: [f32; 32],
    /// Absolute Threshold of Hearing in dB per subband (Terhardt).
    pub ath_db: [f32; 32],
    /// Transient flag per QMF subband (4) — for now always false; the
    /// transient-aware gain envelope is a later phase that needs the
    /// pre-MDCT signal which this module does not see.
    pub transient_per_qmf: [bool; 4],
    /// Perceptual Entropy estimate in bits — the theoretical minimum
    /// bit-count needed to keep NMR ≤ 0 in every subband. If the frame
    /// budget is below this, the allocator must accept audible noise
    /// somewhere.
    pub pe_required_bits: f32,
    /// Fraction of total frame energy that sits above ~16 kHz (bin
    /// 768+). A very low ratio flags a low-pass source (typical of
    /// 128 kbps MP3 re-encodes, FM rips, etc.); the allocator then
    /// stops spending bits on HF that is mostly quantiser noise from
    /// the previous codec.
    pub hf_energy_ratio: f32,
}

/// Compute the psychoacoustic analysis for a full frame of MDCT
/// coefficients. `coefficients` is `SAMPLES_PER_FRAME` long, laid out
/// as concatenated QMF-subband spectra as classic emits them.
///
/// `sample_rate` matters only for the ATH and Bark mapping; ATRAC3 is
/// effectively always 44.1 kHz but we keep it parameterised.
///
/// `channel_idx` (0 or 1) selects which per-channel history slot to
/// use for the transient detector and prediction-based tonality. Mono
/// paths can pass 0.
pub fn compute(coefficients: &[f32], sample_rate: u32, channel_idx: usize) -> PsychoDrive {
    assert_eq!(coefficients.len(), SAMPLES_PER_FRAME);
    let ch = channel_idx.min(1);

    let bark_e = bark_band_energies(coefficients, sample_rate);
    let bark_e_db = bark_e.map(to_db);

    // Stufe B (prediction tonality): regresses HateMe 1.02 → 1.38 even
    // with per-channel state. Hypothesis: dynamic vocal material has
    // fast-changing magnitudes the linear predictor can't track, so
    // tonality collapses to zero on actual tonal content. Stays off.
    // SFM is a gentler measure for real program material.
    let _ = prediction_tonality; // keep the function for later use
    let subband_energy_db = subband_energy_db(coefficients);
    let tonality_per_subband = subband_tonality(coefficients);
    let _ = ch; // silence the warning when prediction_tonality is unused

    let ath_db = subband_ath_db(sample_rate);

    let subband_mask_db = project_masking_to_subbands(
        &bark_e_db,
        &tonality_per_subband,
        &ath_db,
        sample_rate,
    );

    // Perceptual entropy: bits needed per subband ≈ number of coefficients
    // times log2(signal / mask + 1). Sum across subbands.
    let mut pe = 0.0_f32;
    for b in 0..32 {
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        let n_coef = (end - start) as f32;
        let smr = (subband_energy_db[b] - subband_mask_db[b]).max(0.0);
        // ~0.5 bit per 3 dB SMR (rough rate-distortion slope of uniform
        // scalar quantisation); clamped to a sensible range.
        let bits_here = n_coef * (smr / 6.0);
        pe += bits_here;
    }

    let transient_per_qmf = detect_transients(coefficients, ch);
    // Stufe A (temporal masking) stays disabled — see note below.

    // Interchannel masking (stereo-aware pipeline).
    //
    // ATRAC3 at 132 kbps VLC has no format-level joint-stereo coupling:
    // both channels are encoded as independent sound units. Mid/Side
    // coding isn't available to us at the bitstream level. BUT the
    // human auditory system does binaural masking across the two ears
    // — when the same frequency is loud in L, the cochlear response
    // partially masks the corresponding R content. A perceptually
    // equivalent encode can therefore use coarser quantisation on the
    // quieter side of a centered peak without an audible difference.
    //
    // Classic doesn't do this (its comment flatly says "both channels
    // coded independently"). We do it here with a cheap one-direction
    // hand-off: L writes its subband_energy_db to a shared slot when
    // it runs, R reads the slot and raises its mask wherever the L
    // band was significantly louder. Symmetric handling (R→L) would
    // require a two-pass iteration or frame lookahead; the one-way
    // version already covers centered content well enough for a
    // first stereo-aware prototype.
    let mut subband_mask_db = subband_mask_db;
    if ch == 0 {
        // Left channel: publish our energies for the right channel to
        // consult in this same frame.
        CROSS_CHAN_ENERGY.with(|c| *c.borrow_mut() = subband_energy_db);
    } else {
        // Right channel: boost the mask in bands where L was louder.
        // Binaural masking threshold: ~2 dB per dB of L-over-R delta,
        // capped at +6 dB so we never go fully deaf on one side.
        let l_energy = CROSS_CHAN_ENERGY.with(|c| *c.borrow());
        for b in 0..32 {
            let delta = l_energy[b] - subband_energy_db[b];
            // Binaural masking: ~0.5 dB R-mask boost per dB L-over-R
            // delta, threshold +6 dB so only real centering triggers,
            // cap +6 dB so we never deafen one side. Stronger params
            // tested (3 dB threshold / 0.8× slope) produced identical
            // mono scores — our ear_judge is stereo-blind (uses the
            // downmix), so deliberate conservatism until the test
            // suite gains a per-channel or mid/side metric.
            if delta > 6.0 {
                let boost = ((delta - 6.0) * 0.5).min(6.0);
                subband_mask_db[b] += boost;
            }
        }
    }

    // HF-quality flag: sum the MDCT coefficient energies of bins ≥ 768
    // (roughly 16 kHz+) and compare to total energy. A MP3 @ 128 kbps
    // low-passes at ~16 kHz, so its re-encoded HF is almost pure
    // quantiser noise; the ratio drops by ~100× compared to a
    // clean source.
    let mut hf_energy = 0.0_f64;
    let mut total_energy = 1e-20_f64;
    for (i, &c) in coefficients.iter().enumerate() {
        let e = (c as f64) * (c as f64);
        total_energy += e;
        if i >= 768 {
            hf_energy += e;
        }
    }
    let hf_energy_ratio = (hf_energy / total_energy) as f32;

    PsychoDrive {
        bark_energy_db: bark_e_db,
        subband_mask_db,
        subband_energy_db,
        tonality: tonality_per_subband,
        ath_db,
        transient_per_qmf,
        pe_required_bits: pe,
        hf_energy_ratio,
    }
}

// ---------- Transient detection (frame-to-frame energy jump) ----------

/// Detects transients per QMF subband by comparing the current frame's
/// in-band energy to the previous frame's, sharing a thread-local
/// buffer across channels. A 4× jump marks the subband as transient;
/// that flag drives the RDO's HF weight boost and (later) the gain
/// envelope's attack-point count.
fn detect_transients(coefficients: &[f32], channel_idx: usize) -> [bool; 4] {
    let ch = channel_idx.min(1);
    let mut current = [0.0f32; 4];
    for q in 0..4 {
        let mut e = 1e-20_f32;
        for c in &coefficients[q * 256..(q + 1) * 256] {
            e += c * c;
        }
        current[q] = e;
    }
    let prev = PREV_QMF_ENERGY.with(|c| c.borrow()[ch]);
    let mut out = [false; 4];
    for q in 0..4 {
        if current[q] > 4.0 * prev[q].max(1e-10) && current[q] > 1e-3 {
            out[q] = true;
        }
    }
    PREV_QMF_ENERGY.with(|c| c.borrow_mut()[ch] = current);
    out
}

// ---------- Bark band energies ----------

fn bark_band_energies(coefficients: &[f32], sample_rate: u32) -> [f32; N_BARK] {
    // The MDCT layout in classic: `SAMPLES_PER_FRAME` total, grouped
    // into QMF_BANDS=4 subbands of 256 bins each. Each QMF subband
    // covers `sample_rate/8` Hz of the original spectrum, so bin `i`
    // inside QMF band `q` maps to frequency
    //     f = q * (sr/8) + i * (sr/8) / 256   [non-folded]
    // but QMF odd-index bands are frequency-reversed. We fold the
    // per-QMF-subband spectra back to the linear frequency axis and
    // aggregate by the Bark-edge table.
    let sr = sample_rate as f32;
    let qmf_bw = sr / 8.0;      // 5512.5 Hz for 44.1 kHz
    let bin_bw = qmf_bw / 256.0; // ~21.53 Hz per bin

    let mut bark_e = [1e-12_f32; N_BARK];

    for q in 0..4 {
        let reversed = q % 2 == 1;
        for i in 0..256 {
            let coef = coefficients[q * 256 + i];
            let e = coef * coef;
            // bin index into the linearised frequency axis:
            let bin_in_qmf = if reversed { 255 - i } else { i };
            let f_centre = q as f32 * qmf_bw + (bin_in_qmf as f32 + 0.5) * bin_bw;
            let bark = bark_index_from_hz(f_centre);
            bark_e[bark] += e;
        }
    }

    bark_e
}

fn bark_index_from_hz(freq: f32) -> usize {
    for b in 0..N_BARK {
        if freq < BARK_EDGES_HZ[b + 1] {
            return b;
        }
    }
    N_BARK - 1
}

// ---------- Spreading function (Schroeder) ----------

/// Schroeder's two-slope spreading function for Bark bands. Returns the
/// masking contribution in dB from a source at `src` onto a target at
/// `dst`, both as Bark-band indices. Positive offset means slope is
/// below the source; we apply it as a subtraction from the source
/// level.
fn spread_db(src_bark: usize, dst_bark: usize) -> f32 {
    let dz = dst_bark as f32 - src_bark as f32;
    if dz >= 0.0 {
        // Upper skirt: steep — 27 dB per Bark above the source.
        27.0 * dz
    } else {
        // Lower skirt: gentler — 15 dB per Bark below the source.
        15.0 * (-dz)
    }
}

fn project_masking_to_subbands(
    bark_energy_db: &[f32; N_BARK],
    tonality: &[f32; 32],
    ath_db: &[f32; 32],
    sample_rate: u32,
) -> [f32; 32] {
    let sr = sample_rate as f32;

    // Step 1: For each Bark band, compute the global masking floor that
    // this band induces on every other band (max across sources).
    let mut bark_mask_db = [-120.0_f32; N_BARK];
    for src in 0..N_BARK {
        let src_level = bark_energy_db[src];
        if !src_level.is_finite() || src_level < -100.0 {
            continue;
        }
        for dst in 0..N_BARK {
            // Start from source level, subtract spreading attenuation
            // and a signal-to-mask margin that depends on whether the
            // source is tonal (18 dB) or noisy (6 dB). We don't have
            // per-Bark tonality; take the mean of overlapping subbands
            // as a proxy.
            let smr = bark_smr_db(src, tonality);
            let contrib = src_level - spread_db(src, dst) - smr;
            if contrib > bark_mask_db[dst] {
                bark_mask_db[dst] = contrib;
            }
        }
    }

    // Step 2: Project the per-Bark mask floor onto ATRAC3 subbands by
    // taking the maximum mask over the Bark bands that overlap the
    // subband's frequency range. Then union with ATH.
    let qmf_bw = sr / 8.0;
    let bin_bw = qmf_bw / 256.0;
    let mut out = [-120.0_f32; 32];
    for subb in 0..32 {
        let start = ATRAC3_SUBBAND_TAB[subb];
        let end = ATRAC3_SUBBAND_TAB[subb + 1];
        // Find ATRAC3 subband's Hz extent via its first/last bins.
        let (lo_bin_global, hi_bin_global) = (start, end - 1);
        let (f_lo, f_hi) = (
            subband_bin_to_hz(lo_bin_global, qmf_bw, bin_bw),
            subband_bin_to_hz(hi_bin_global, qmf_bw, bin_bw),
        );
        let (f_min, f_max) = if f_lo < f_hi {
            (f_lo, f_hi)
        } else {
            (f_hi, f_lo)
        };
        let b_lo = bark_index_from_hz(f_min);
        let b_hi = bark_index_from_hz(f_max);
        let mut m = -120.0_f32;
        for b in b_lo..=b_hi {
            if bark_mask_db[b] > m {
                m = bark_mask_db[b];
            }
        }
        // ATH floor
        if ath_db[subb] > m {
            m = ath_db[subb];
        }
        out[subb] = m;
    }
    out
}

fn subband_bin_to_hz(global_bin: usize, qmf_bw: f32, bin_bw: f32) -> f32 {
    // global_bin is an index into the full SAMPLES_PER_FRAME layout
    // (concatenated QMF subbands). Map it back to a linear Hz axis.
    let q = global_bin / 256;
    let local = global_bin % 256;
    let reversed = q % 2 == 1;
    let bin_in_qmf = if reversed { 255 - local } else { local };
    q as f32 * qmf_bw + (bin_in_qmf as f32 + 0.5) * bin_bw
}

fn bark_smr_db(bark: usize, subband_tonality: &[f32; 32]) -> f32 {
    // Find subbands whose centre falls in this Bark band, average
    // their tonality, and blend 18 dB (tonal) and 6 dB (noise).
    // Cheap approximation: sample 4 subbands near the matching position.
    let bark_centre_hz = 0.5 * (BARK_EDGES_HZ[bark] + BARK_EDGES_HZ[bark + 1]);
    // ATRAC3 linear freq-per-subband on 44.1 kHz is about 689 Hz/band,
    // so:
    let subb_est = ((bark_centre_hz / 689.0) as usize).min(31);
    let lo = subb_est.saturating_sub(1);
    let hi = (subb_est + 2).min(32);
    let mut sum = 0.0_f32;
    let mut count = 0.0_f32;
    for s in lo..hi {
        sum += subband_tonality[s];
        count += 1.0;
    }
    let tonal = if count > 0.0 { sum / count } else { 0.5 };
    // Blend: pure tonal → 18 dB, pure noise → 6 dB. Linear.
    6.0 + 12.0 * tonal.clamp(0.0, 1.0)
}

// ---------- ATH (Terhardt) ----------

fn ath_db_at(freq: f32) -> f32 {
    // Terhardt 1979 ATH curve in dB SPL, rescaled to dBFS assuming a
    // playback SPL reference where full-scale sine ≈ 96 dB SPL. The
    // offset is approximate; what we care about is relative shape.
    let khz = freq / 1000.0;
    let khz = khz.max(0.02); // avoid nonsense at DC
    let spl = 3.64 * khz.powf(-0.8)
        - 6.5 * (-0.6 * (khz - 3.3).powi(2)).exp()
        + 1e-3 * khz.powi(4);
    // Rescale: ATH at 3-4 kHz is about 0 dB SPL → that corresponds to
    // roughly -96 dBFS in our digital domain. Shift accordingly.
    spl - 96.0
}

fn subband_ath_db(sample_rate: u32) -> [f32; 32] {
    let sr = sample_rate as f32;
    let qmf_bw = sr / 8.0;
    let bin_bw = qmf_bw / 256.0;
    let mut out = [0.0_f32; 32];
    for b in 0..32 {
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        // Use the centre bin of the subband for ATH.
        let mid_global = (start + end) / 2;
        let f = subband_bin_to_hz(mid_global, qmf_bw, bin_bw);
        out[b] = ath_db_at(f);
    }
    out
}

// ---------- Per-subband energy and tonality ----------

fn subband_energy_db(coefficients: &[f32]) -> [f32; 32] {
    let mut out = [-120.0_f32; 32];
    for b in 0..32 {
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        let mut e = 1e-20_f32;
        for c in &coefficients[start..end] {
            e += c * c;
        }
        out[b] = to_db(e / (end - start) as f32);
    }
    out
}

/// Prediction-based tonality (MPEG-1/2 Psychoacoustic Model 2).
///
/// Compute per-subband the "unpredictability measure" by linearly
/// extrapolating the magnitude from the two previous frames and
/// comparing to the actual. Then map to a tonality coefficient in
/// [0, 1] via the standard formula `tonality = -0.299 - 0.43·log10(u)`,
/// clamped. Requires thread-local 2-frame history; first two frames
/// fall back to an SFM-based estimate so the encoder warms up gracefully.
fn prediction_tonality(coefficients: &[f32], channel_idx: usize) -> [f32; 32] {
    let ch = channel_idx.min(1);
    let prev: [f32; 32] = PREV_MAG.with(|c| c.borrow()[ch]);
    let prev_prev: [f32; 32] = PREV_PREV_MAG.with(|c| c.borrow()[ch]);

    let mut cur_mag = [0.0_f32; 32];
    let mut out = [0.0_f32; 32];
    let warmup = prev_prev.iter().all(|&v| v == 0.0);

    for b in 0..32 {
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        // Per-subband magnitude = sqrt(mean-energy). Simpler than the
        // polar-coordinate dual-path of exact PM2 but captures the
        // same prediction signal for our band-aggregated context.
        let mut e = 0.0_f32;
        for c in &coefficients[start..end] {
            e += c * c;
        }
        let mag = (e / (end - start) as f32).sqrt().max(1e-20);
        cur_mag[b] = mag;

        if warmup {
            // Warm-up: fall back to SFM-based tonality until two
            // frames of history are built up. Same code path as the
            // old `subband_tonality` but inlined for clarity.
            let slice = &coefficients[start..end];
            let mut log_sum = 0.0_f32;
            let mut arith = 0.0_f32;
            for c in slice {
                let sq = c * c + 1e-20;
                log_sum += sq.ln();
                arith += sq;
            }
            let n = slice.len() as f32;
            let sfm = (log_sum / n).exp() / (arith / n + 1e-20);
            out[b] = (-sfm.max(1e-10).log10() / 2.0).clamp(0.0, 1.0);
            continue;
        }

        // Linear prediction: if magnitude changes at a constant rate,
        // predict = 2*prev − prev_prev. Pure sine: prediction is exact,
        // unpredictability → 0. White noise: prediction is random,
        // unpredictability → ~1.
        let predicted = (2.0 * prev[b] - prev_prev[b]).max(0.0);
        let denom = mag + predicted + 1e-20;
        let unpred = ((mag - predicted).abs() / denom).clamp(1e-6, 1.0);
        // MPEG formula: tonality = -0.299 - 0.43 * ln(u), clamped
        // to [0, 1]. A typical pure sine gives u ≈ 0.01, yielding
        // tonality ≈ 1.6 — we clamp at 1. White noise u ≈ 0.6 gives
        // tonality ≈ -0.08 — we clamp at 0.
        let t = -0.299 - 0.43 * unpred.ln();
        out[b] = t.clamp(0.0, 1.0);
    }

    // Shift history (per channel).
    PREV_PREV_MAG.with(|c| c.borrow_mut()[ch] = prev);
    PREV_MAG.with(|c| c.borrow_mut()[ch] = cur_mag);
    out
}

#[allow(dead_code)]
fn subband_tonality(coefficients: &[f32]) -> [f32; 32] {
    let mut out = [0.0_f32; 32];
    for b in 0..32 {
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        let slice = &coefficients[start..end];
        if slice.is_empty() {
            continue;
        }
        // Spectral Flatness Measure on magnitude-squared:
        //   SFM = geom_mean(|X|^2) / arith_mean(|X|^2)
        // Low SFM → tonal, high SFM → noise. tonality = 1 - SFM.
        let eps = 1e-20_f32;
        let mut log_sum = 0.0_f32;
        let mut arith = 0.0_f32;
        for c in slice {
            let e = c * c + eps;
            log_sum += e.ln();
            arith += e;
        }
        let n = slice.len() as f32;
        let geom = (log_sum / n).exp();
        let am = arith / n;
        let sfm = geom / am;
        // Map SFM to tonality via a soft curve:
        //   tonality = clamp(-log10(sfm) / 2, 0, 1)
        // (sfm=0.01 → 1.0, sfm=0.1 → 0.5, sfm=1 → 0)
        let t = (-sfm.max(1e-10).log10() / 2.0).clamp(0.0, 1.0);
        out[b] = t;
    }
    out
}

fn to_db(e: f32) -> f32 {
    10.0 * (e.max(1e-20)).log10()
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_frame(freq_hz: f32, sample_rate: u32, amp: f32) -> Vec<f32> {
        // Crude stand-in: directly place a peak in one subband bin at
        // the target frequency. The analysis only needs shaped
        // coefficients, not a real MDCT output, for these smoke tests.
        let mut c = vec![0.0f32; SAMPLES_PER_FRAME];
        let sr = sample_rate as f32;
        let bin_bw = sr / 8.0 / 256.0;
        let q = (freq_hz / (sr / 8.0)).floor() as usize;
        let q = q.min(3);
        let reversed = q % 2 == 1;
        let local = ((freq_hz - q as f32 * sr / 8.0) / bin_bw) as usize;
        let local = local.min(255);
        let idx_in_global = q * 256 + if reversed { 255 - local } else { local };
        c[idx_in_global] = amp;
        c
    }

    #[test]
    fn compute_finishes_on_silence() {
        let c = vec![0.0f32; SAMPLES_PER_FRAME];
        let p = compute(&c, 44100, 0);
        assert!(p.pe_required_bits.is_finite());
        // silence → all subband energies at floor
        for e in &p.subband_energy_db {
            assert!(*e < -100.0);
        }
    }

    #[test]
    fn compute_on_1khz_sine_flags_bark_3() {
        let c = sine_frame(1000.0, 44100, 1.0);
        let p = compute(&c, 44100, 0);
        // 1 kHz lives in Bark band 8 (920-1080 Hz).
        let loudest = p
            .bark_energy_db
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(loudest, 8, "1 kHz should peak in Bark 8, got {}", loudest);
    }

    #[test]
    fn tonality_on_sine_is_high() {
        let c = sine_frame(2000.0, 44100, 1.0);
        let p = compute(&c, 44100, 0);
        let max_t = p.tonality.iter().cloned().fold(0.0, f32::max);
        assert!(max_t > 0.5, "sine tonality should be > 0.5, got {}", max_t);
    }
}
