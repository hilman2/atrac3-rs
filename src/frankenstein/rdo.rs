//! Lagrangian rate-distortion allocator.
//!
//! Replaces classic's Phase 4 + Post-Promote + Robin Hood chain with a
//! single global optimisation:
//!
//!     minimise    Σ_b  w(b) · distortion(b, tbl_b, sf_b)
//!     subject to  Σ_b  bits(b, tbl_b, sf_b) ≤ target_bits
//!
//! solved by introducing a Lagrange multiplier λ and picking, per band
//! and per λ, the (tbl, sf) that minimises
//!
//!     bits + λ · w · distortion
//!
//! `w(b)` is the perceptual weight from the psychoacoustic driver —
//! roughly *"how much of the quantisation noise in this band would be
//! audible above the masking floor"*. Bands under ATH or thoroughly
//! masked by their neighbours get w ≈ 0; bands with a tonal signal
//! peeking far above the masking curve get w ≈ high.

use anyhow::{Result, ensure};

use crate::atrac3::{
    SAMPLES_PER_FRAME,
    quant::{
        ATRAC3_SUBBAND_TAB, QuantizedSubband, SpectrumEncoding,
        candidate_total_bits, fixed_sound_unit_bits, optimal_sf_index_for_peak,
        quantize_subband,
    },
    sound_unit::{CodingMode, SpectralUnit},
};

use super::analysis::PsychoDrive;

/// A pre-computed quantisation candidate for a single (band, tbl, sf).
#[derive(Debug, Clone)]
struct Candidate {
    q: QuantizedSubband,
    bits: usize,
    mse: f32,
}

/// Candidate set for one band: index 0 = "drop the band" (tbl=0), then
/// one or more real quantisation choices per tbl.
struct BandCandidates {
    drop: Candidate,          // tbl=0, the always-present fallback
    real: Vec<Candidate>,     // (tbl=1..7) with the sf-delta that minimises mse
}

/// Main entry: solve the RDO problem for one frame.
pub fn allocate(
    coefficients: &[f32],
    coding_mode: CodingMode,
    target_bits: usize,
    psycho: &PsychoDrive,
) -> Result<SpectrumEncoding> {
    ensure!(coefficients.len() == SAMPLES_PER_FRAME);
    ensure!(target_bits > 0);

    let debug = std::env::var("FRANK_DEBUG").is_ok();

    // Determine the active band extent: cheapest way is to look at
    // subband_energy_db and cut off where all remaining bands are at
    // floor. But we keep it simple: active = all bands whose energy is
    // above ATH by at least 3 dB (otherwise we spend overhead encoding
    // pure silence).
    let mut num_active = 1usize;
    for b in 0..32 {
        // iter9: drop the ATH+3 headroom requirement and take ATH
        // itself as the audibility threshold. The +3 was being
        // over-conservative: it excluded bands that sat near hearing
        // threshold but would still contribute to the perceived
        // timbre, and those then counted as -inf dB deltas in the
        // octave balance score.
        let audible = psycho.subband_energy_db[b] > psycho.ath_db[b];
        if audible {
            num_active = b + 1;
        }
    }
    if num_active == 0 {
        num_active = 1;
    }

    let fixed_overhead = fixed_sound_unit_bits(num_active);
    let mut available = target_bits.saturating_sub(fixed_overhead);

    // Build candidates per band. This is the only heavy compute — ~32
    // bands × 7 tbls × 3 sf-deltas = ~672 quantise calls per frame.
    let candidates = build_candidates(coefficients, coding_mode, num_active)?;

    // Perceptual weight per band. Three shaping terms:
    //  - (signal − mask), clamped to [0.5, 25] dB. Clamp bottom so no
    //    live band gets literally zero weight; clamp top so a 45-dB-
    //    above-mask bass drum doesn't hog the entire bit budget.
    //  - tonality_boost (1.0 … 2.0): tonal signal needs more accuracy
    //    than noise of the same loudness.
    //  - transient HF dampen (× 0.5 for bands ≥ 16 in a transient-flagged
    //    QMF subband): pre-echo comes from precisely-coded HF
    //    coefficients ringing before the attack. Slightly coarser HF
    //    in transient frames trades a tiny SNR hit for big pre-echo
    //    reduction.
    // Input-aware gate: if the frame's HF energy ratio is vanishingly
    // small, the source was almost certainly low-passed upstream
    // (typically 128 kbps MP3). Spending bits on its "HF" is just
    // preserving the previous codec's quantiser noise. We detect
    // per-frame and redirect the budget to Mid/Presence where the
    // real music lives.
    let low_pass_source = psycho.hf_energy_ratio < 0.001;

    let mut weights = [0.0_f32; 32];
    for b in 0..num_active {
        let smr = psycho.subband_energy_db[b] - psycho.subband_mask_db[b];
        let above_mask = smr.max(0.5).min(25.0);
        let tonality_boost = 1.0 + psycho.tonality[b];
        let mut w = above_mask * tonality_boost;

        let qmf_band = (b / 8).min(3);
        if psycho.transient_per_qmf[qmf_band] && b >= 16 {
            w *= 0.5;
        }
        // Low-pass source (128 kbps MP3 etc.): upstream HF is mostly
        // quantiser noise. Shape the weights in four tiers — but
        // keeping in mind ATRAC3's non-uniform band layout!
        //   Band 16-19 ≈  4.1 -  6.2 kHz  — VOCAL CLARITY (Presence)
        //   Band 20-21 ≈  6.2 -  8.3 kHz  — VOCAL SIBILANCE
        //   Band 22-27 ≈  8.3 - 13.8 kHz  — Brilliance
        //   Band 28-29 ≈ 13.8 - 15.1 kHz  — top Brilliance
        //   Band 30-31 ≈ 16.5 - 22.0 kHz  — Air
        //
        //  - Air (≥ 30):             ×0.05 — pure upstream noise, toss.
        //  - Upper Brilliance (28+): ×0.3  — residual; whisper.
        //  - Brilliance (22-27):     ×0.8  — keep it, slightly demote.
        //  - Vocal range (16-21):    ×1.3  — BOOST. Presence + sibilance.
        //    Without this voices sound noticeably "dumpfer" than classic;
        //    measured via waveform_dive.py with -0.7 dB in the 4-6 kHz
        //    band even when the rest of the spectrum matches.
        if low_pass_source {
            if b >= 30 {
                w *= 0.3;
            } else if b >= 28 {
                w *= 0.5;
            } else if b >= 22 {
                w *= 1.0;
            } else if b >= 16 {
                w *= 1.3;
            }
        } else {
            // iter7: on clean sources the Presence/Brilliance bands
            // are real signal, not upstream noise. Boost them by 1.2
            // so the RDO prioritises them against Mid on the margin.
            // Measured gap to classic on classic_30s was dominated by
            // NMR over mask + octave balance in exactly these bands.
            if (22..=27).contains(&b) {
                w *= 1.3;
            } else if (16..=21).contains(&b) {
                w *= 1.15;
            }
        }
        weights[b] = w;
    }

    // HF safety floor: Presence/Brilliance bands whose signal sits
    // well above ATH are pinned to a minimum tbl so the RDO can't
    // starve them into `drop` when an LF band looks more efficient.
    // Mirrors classic's MIN_TBL intent but is now driven by psycho's
    // ATH headroom rather than a hard-coded band index.
    let mut hf_floor = [0u8; 32];
    for b in 0..num_active {
        let headroom = psycho.subband_energy_db[b] - psycho.ath_db[b];
        if headroom <= 6.0 {
            continue;
        }
        // Iter2: floors on every source class. Lossy upstream doesn't
        // mean "the HF doesn't exist" — it means HF content is flat
        // and quiet. Even a tbl=1 encode preserves the envelope and
        // saves the octave-balance penalty we were taking for dropping
        // the band to silence.
        //
        //   Band 16-21 (≈ 4.1-8.3 kHz): vocal-clarity floor tbl=2
        //   Band 22-27 (≈ 8.3-13.8 kHz): brilliance floor tbl=1
        //   Band 28-29 (≈ 13.8-15.1 kHz): upper brilliance, tbl=1
        //   Band 30-31 (≈ 16.5+ kHz): air, tbl=1 with stricter
        //                              headroom so we don't spend
        //                              bits on pure noise.
        if b >= 30 {
            if headroom > 10.0 {
                hf_floor[b] = if !low_pass_source { 2 } else { 1 };
            }
        } else if b >= 28 {
            hf_floor[b] = if !low_pass_source && headroom > 12.0 { 2 } else { 1 };
        } else if b >= 24 {
            // iter6: split Brilliance in two halves.
            //   24-27 (9.6-13.8 kHz): on clean tbl=3 (7 mantissa levels),
            //                         matches Sony's air-band resolution
            //   on lossy tbl=1.
            hf_floor[b] = if !low_pass_source { 3 } else { 1 };
        } else if b >= 22 {
            //   22-23 (8.3-9.6 kHz): tbl=2 on clean, tbl=1 on lossy.
            hf_floor[b] = if !low_pass_source { 2 } else { 1 };
        } else if b >= 16 {
            hf_floor[b] = 2;
        }
    }

    if debug {
        eprintln!("[frank] num_active={} overhead={} avail={} pe_bits={:.0}",
                  num_active, fixed_overhead, available, psycho.pe_required_bits);
        for b in 0..num_active {
            eprintln!("  b{:2}  sig={:+7.1}  mask={:+7.1}  ath={:+7.1}  ton={:.2}  w={:.2}  floor={}",
                b, psycho.subband_energy_db[b], psycho.subband_mask_db[b],
                psycho.ath_db[b], psycho.tonality[b], weights[b], hf_floor[b]);
        }
        eprintln!("[frank] transients={:?}", psycho.transient_per_qmf);
    }

    let (tbl_choice, sf_choice, used_bits) =
        lambda_search(&candidates, &weights, &hf_floor, num_active, available);

    if debug {
        eprintln!("[frank] after λ: used={}/{} tbls={:?}",
            used_bits, available, &tbl_choice[..num_active]);
    }

    // If we overshot the budget due to quantisation of the candidate
    // set, drop the least-valuable band until we fit. This is the
    // safety net for pathological frames.
    let (tbl_choice, sf_choice, used_bits) =
        tighten_to_budget(&candidates, &weights, &hf_floor, num_active, available, tbl_choice, sf_choice, used_bits);

    // Emit the SpectrumEncoding.
    assemble_encoding(coefficients, coding_mode, num_active, &candidates, &tbl_choice, &sf_choice, used_bits, fixed_overhead)
}

fn build_candidates(
    coefficients: &[f32],
    coding_mode: CodingMode,
    num_active: usize,
) -> Result<[Option<BandCandidates>; 32]> {
    let mut out: [Option<BandCandidates>; 32] = Default::default();
    for b in 0..num_active {
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        let slice = &coefficients[start..end];

        // "drop" candidate
        let drop_q = QuantizedSubband::uncoded(slice);
        let drop_bits = candidate_total_bits(&drop_q);
        let drop_mse = drop_q.mse;
        let drop = Candidate { q: drop_q, bits: drop_bits, mse: drop_mse };

        let peak = slice.iter().map(|c| c.abs()).fold(0.0f32, f32::max);

        // Mirror Phase-5 delta window for parity with classic.
        let (start_delta, max_delta) = if b >= 28 {
            (2i8, 5i8)
        } else if b >= 22 {
            (1i8, 4i8)
        } else if b >= 20 {
            (1i8, 3i8)
        } else {
            (0i8, 3i8)
        };

        let mut real = Vec::with_capacity(7);
        for tbl in 1..=7u8 {
            let sf_center = optimal_sf_index_for_peak(peak, tbl);
            let mut best: Option<Candidate> = None;
            for d in start_delta..=max_delta {
                let sf_try = (sf_center as i8 + d).clamp(0, 63) as u8;
                if let Ok(q) = quantize_subband(slice, tbl, sf_try, coding_mode) {
                    let bits = candidate_total_bits(&q);
                    let mse = q.mse;
                    let is_better = match &best {
                        None => true,
                        Some(prev) => mse < prev.mse,
                    };
                    if is_better {
                        best = Some(Candidate { q, bits, mse });
                    }
                }
            }
            if let Some(c) = best {
                real.push(c);
            }
        }

        out[b] = Some(BandCandidates { drop, real });
    }
    Ok(out)
}

/// For a given λ, pick per band the candidate minimising bits + λ·w·mse.
/// Returns (chosen tbl per band, chosen candidate index into `real`,
/// total bits used).
fn pick_for_lambda(
    candidates: &[Option<BandCandidates>; 32],
    weights: &[f32; 32],
    hf_floor: &[u8; 32],
    num_active: usize,
    lambda: f32,
) -> ([u8; 32], [usize; 32], usize) {
    let mut tbl_choice = [0u8; 32];
    let mut sf_choice = [0usize; 32];
    let mut total_bits = 0usize;

    for b in 0..num_active {
        let Some(bc) = candidates[b].as_ref() else { continue };
        let w = weights[b];
        let min_tbl = hf_floor[b];

        // Consider the drop candidate only if there's no safety floor
        // for this band.
        let (mut best_score, mut best_tbl, mut best_idx, mut best_bits) = if min_tbl == 0 {
            (bc.drop.bits as f32 + lambda * w * bc.drop.mse, 0u8, 0usize, bc.drop.bits)
        } else {
            (f32::INFINITY, 0u8, 0usize, 0usize)
        };

        for (i, cand) in bc.real.iter().enumerate() {
            if cand.q.table_index < min_tbl {
                continue;
            }
            let score = cand.bits as f32 + lambda * w * cand.mse;
            if score < best_score {
                best_score = score;
                best_tbl = cand.q.table_index;
                best_idx = i;
                best_bits = cand.bits;
            }
        }
        tbl_choice[b] = best_tbl;
        sf_choice[b] = best_idx;
        total_bits += best_bits;
    }

    (tbl_choice, sf_choice, total_bits)
}

fn lambda_search(
    candidates: &[Option<BandCandidates>; 32],
    weights: &[f32; 32],
    hf_floor: &[u8; 32],
    num_active: usize,
    target: usize,
) -> ([u8; 32], [usize; 32], usize) {
    // Score per band per λ:   bits + λ · w · mse
    // As λ grows the distortion term dominates, so higher-tbl choices
    // (more bits, less mse) become preferred. Thus bits(λ) is monotone
    // INCREASING in λ. Objective: find the LARGEST λ whose total bits
    // still fit the budget — that's the allocation with the most
    // quality we can afford.
    let mut lo = 1e-6_f32;   // tiny λ → minimises bits → "drop everything"
    let mut hi = 1e9_f32;    // large λ → minimises mse   → "max tbl everywhere"

    let (tl_lo, sf_lo, bits_lo) = pick_for_lambda(candidates, weights, hf_floor, num_active, lo);
    let (tl_hi, sf_hi, bits_hi) = pick_for_lambda(candidates, weights, hf_floor, num_active, hi);

    // Happy path: even max-quality fits. Take it.
    if bits_hi <= target {
        return (tl_hi, sf_hi, bits_hi);
    }
    // Safety path: even the minimum (all drops) overshoots — shouldn't
    // happen because drop candidates are tiny, but fall back to drops.
    if bits_lo > target {
        return (tl_lo, sf_lo, bits_lo);
    }

    // Bisection: lo keeps the best feasible (bits ≤ target), hi the
    // infeasible side (bits > target). Tighten until they meet.
    let mut best = (tl_lo, sf_lo, bits_lo);
    for _ in 0..24 {
        let mid = (lo.ln() + hi.ln()) * 0.5;
        let lam = mid.exp();
        let (tl, sf, bits) = pick_for_lambda(candidates, weights, hf_floor, num_active, lam);
        if bits <= target {
            // Feasible — this is our best-so-far. Try to push higher λ
            // for even more bits.
            best = (tl, sf, bits);
            lo = lam;
        } else {
            // Infeasible — pull λ back down.
            hi = lam;
        }
        if (hi / lo) < 1.05 {
            break;
        }
    }
    best
}

fn tighten_to_budget(
    candidates: &[Option<BandCandidates>; 32],
    weights: &[f32; 32],
    hf_floor: &[u8; 32],
    num_active: usize,
    target: usize,
    mut tbl_choice: [u8; 32],
    mut sf_choice: [usize; 32],
    mut used_bits: usize,
) -> ([u8; 32], [usize; 32], usize) {
    // Fallback tightener: if we're still over budget due to ties in the
    // λ-search, demote the band with the lowest perceptual cost per bit.
    // HF-floor bands are still demotable but only down to their floor.
    while used_bits > target {
        let mut best_band = None;
        let mut best_score = f32::INFINITY;
        for b in 0..num_active {
            let Some(bc) = candidates[b].as_ref() else { continue };
            if tbl_choice[b] == 0 {
                continue;
            }
            // Current and demoted candidate indexes:
            let cur_idx = sf_choice[b];
            let cur = &bc.real[cur_idx];
            let min_tbl = hf_floor[b];
            // Find next-smaller tbl that still respects the HF floor.
            // If min_tbl > 0 and current is already at floor, skip —
            // this band can't be demoted further.
            let mut alt_bits = if min_tbl == 0 { bc.drop.bits } else { usize::MAX };
            let mut alt_mse = if min_tbl == 0 { bc.drop.mse } else { f32::INFINITY };
            let mut alt_idx: Option<usize> = None;
            for (i, cand) in bc.real.iter().enumerate() {
                if cand.q.table_index < cur.q.table_index
                    && cand.q.table_index >= min_tbl
                    && cand.bits < cur.bits {
                    if alt_idx.is_none() || cand.bits > alt_bits {
                        alt_bits = cand.bits;
                        alt_mse = cand.mse;
                        alt_idx = Some(i);
                    }
                }
            }
            if alt_idx.is_none() && min_tbl > 0 {
                continue;  // already at HF floor, can't demote
            }
            // Cost of demotion = mse rise × weight, per bit saved.
            let bits_saved = cur.bits.saturating_sub(alt_bits);
            if bits_saved == 0 {
                continue;
            }
            let mse_rise = (alt_mse - cur.mse).max(0.0);
            let score = (mse_rise * weights[b] + 1e-12) / bits_saved as f32;
            if score < best_score {
                best_score = score;
                best_band = Some((b, alt_idx, alt_bits, alt_mse));
            }
        }
        match best_band {
            Some((b, alt_idx, alt_bits, _alt_mse)) => {
                used_bits = used_bits.saturating_sub(
                    candidates[b].as_ref().unwrap().real[sf_choice[b]].bits,
                ) + alt_bits;
                match alt_idx {
                    Some(i) => {
                        tbl_choice[b] = candidates[b].as_ref().unwrap().real[i].q.table_index;
                        sf_choice[b] = i;
                    }
                    None => {
                        tbl_choice[b] = 0;
                        sf_choice[b] = 0; // unused when tbl=0
                    }
                }
            }
            None => break,
        }
    }
    (tbl_choice, sf_choice, used_bits)
}

fn assemble_encoding(
    coefficients: &[f32],
    coding_mode: CodingMode,
    num_active: usize,
    candidates: &[Option<BandCandidates>; 32],
    tbl_choice: &[u8; 32],
    sf_choice: &[usize; 32],
    _used_bits: usize,
    _fixed_overhead: usize,
) -> Result<SpectrumEncoding> {
    let mut quantized_subbands = Vec::with_capacity(32);
    let mut reconstructed = vec![0.0f32; SAMPLES_PER_FRAME];
    let mut spectral_subbands = Vec::with_capacity(num_active);
    let mut payload_bits = 0usize;

    for b in 0..num_active {
        let Some(bc) = candidates[b].as_ref() else {
            let slice = &coefficients[ATRAC3_SUBBAND_TAB[b]..ATRAC3_SUBBAND_TAB[b + 1]];
            let uncoded = QuantizedSubband::uncoded(slice);
            spectral_subbands.push(uncoded.spectral_subband(coding_mode)?);
            quantized_subbands.push(uncoded);
            continue;
        };
        let chosen = if tbl_choice[b] == 0 {
            bc.drop.q.clone()
        } else {
            bc.real[sf_choice[b]].q.clone()
        };
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        let recon = chosen.dequantized(end - start)?;
        reconstructed[start..end].copy_from_slice(&recon);
        payload_bits += chosen.payload_bits;
        spectral_subbands.push(chosen.spectral_subband(coding_mode)?);
        quantized_subbands.push(chosen);
    }
    for b in num_active..32 {
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        reconstructed[start..end].fill(0.0);
        quantized_subbands.push(QuantizedSubband::uncoded(&coefficients[start..end]));
    }

    // Strip trailing uncoded bands from the spectral_subband list so the
    // bitstream writer emits a compact header.
    while spectral_subbands.len() > 1
        && spectral_subbands.last().map(|s| s.table_index == 0).unwrap_or(false)
    {
        spectral_subbands.pop();
    }

    let mse = mean_square(coefficients, &reconstructed);
    Ok(SpectrumEncoding {
        spectral_unit: SpectralUnit {
            coding_mode,
            subbands: spectral_subbands,
        },
        quantized_subbands,
        reconstructed,
        payload_bits,
        mse,
    })
}

fn mean_square(a: &[f32], b: &[f32]) -> f32 {
    let mut s = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let d = (*x - *y) as f64;
        s += d * d;
    }
    (s / a.len() as f64) as f32
}
