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

use crate::atrac3::{SAMPLES_PER_FRAME, quant::ATRAC3_SUBBAND_TAB};

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
}

/// Compute the psychoacoustic analysis for a full frame of MDCT
/// coefficients. `coefficients` is `SAMPLES_PER_FRAME` long, laid out
/// as concatenated QMF-subband spectra as classic emits them.
///
/// `sample_rate` matters only for the ATH and Bark mapping; ATRAC3 is
/// effectively always 44.1 kHz but we keep it parameterised.
pub fn compute(coefficients: &[f32], sample_rate: u32) -> PsychoDrive {
    assert_eq!(coefficients.len(), SAMPLES_PER_FRAME);

    let bark_e = bark_band_energies(coefficients, sample_rate);
    let bark_e_db = bark_e.map(to_db);

    let tonality_per_subband = subband_tonality(coefficients);
    let subband_energy_db = subband_energy_db(coefficients);

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

    PsychoDrive {
        bark_energy_db: bark_e_db,
        subband_mask_db,
        subband_energy_db,
        tonality: tonality_per_subband,
        ath_db,
        transient_per_qmf: [false; 4],
        pe_required_bits: pe,
    }
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
        let p = compute(&c, 44100);
        assert!(p.pe_required_bits.is_finite());
        // silence → all subband energies at floor
        for e in &p.subband_energy_db {
            assert!(*e < -100.0);
        }
    }

    #[test]
    fn compute_on_1khz_sine_flags_bark_3() {
        let c = sine_frame(1000.0, 44100, 1.0);
        let p = compute(&c, 44100);
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
        let p = compute(&c, 44100);
        let max_t = p.tonality.iter().cloned().fold(0.0, f32::max);
        assert!(max_t > 0.5, "sine tonality should be > 0.5, got {}", max_t);
    }
}
