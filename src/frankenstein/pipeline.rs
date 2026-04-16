//! Thin orchestration layer: `analysis → rdo → emit`.
//!
//! `build_spectral_unit` matches the signature that classic's
//! `build_spectral_unit_budgeted` exposes, so classic can route to us
//! via the `ATRAC3_ENGINE` env var with zero call-site changes.

use anyhow::Result;

use crate::atrac3::{
    quant::{SearchOptions, SpectrumEncoding},
    sound_unit::CodingMode,
};

use super::{analysis, rdo};

pub fn build_spectral_unit(
    coefficients: &[f32],
    coding_mode: CodingMode,
    options: SearchOptions,
    target_bits: usize,
) -> Result<SpectrumEncoding> {
    // ATRAC3 is effectively always 44.1 kHz; keep it parameterised for
    // later tools.
    let sample_rate = 44100u32;
    let channel_idx = options.channel_idx.min(1);

    let psycho = analysis::compute(coefficients, sample_rate, channel_idx);

    // #2 HF noise injection experiment (v31/v31b/v31c) rolled back.
    //
    // Concept: noise-like HF bands (low crest factor, upstream codec
    // quantiser residue) can be replaced with phase-randomised
    // Gaussian at matching RMS; the quantiser saves bits because
    // the replacement has narrower magnitude distribution, and
    // perceptually the decoded result is the same hiss.
    //
    // Why it doesn't work in our setting: vocal sibilance and cymbal
    // decay ALSO have low crest on narrow ATRAC3 subbands (peak/rms
    // typically 2.3-2.8). Without a PNS-style side-channel flag
    // telling the encoder "this specific band is safe to replace
    // with noise", we can't distinguish music noise from codec noise
    // with pure coefficient statistics. All three thresholds tested
    // (loose/medium/strict) regressed the benchmark by 7-28 points
    // and zoomed-snare HF-hole widened from -3.0 to -3.6 dB.
    //
    // A proper implementation would need ATRAC3 to carry a
    // per-band-per-frame PNS flag — not in the format. Keeping the
    // inject_hf_noise function in the file (unused) so the
    // reasoning is findable next time someone looks at this.
    let _ = inject_hf_noise;

    rdo::allocate(coefficients, coding_mode, target_bits, &psycho)
}

fn inject_hf_noise<'a>(
    coefficients: &'a [f32],
    psycho: &analysis::PsychoDrive,
    channel_idx: usize,
) -> std::borrow::Cow<'a, [f32]> {
    use crate::atrac3::quant::ATRAC3_SUBBAND_TAB;

    // Per-band replacement decision: high SFM + HF band + audible
    // enough to matter. Apply only to bands ≥ 20 (Upper-Mid top
    // upward); leaving Presence floor 16-19 alone preserves vocal
    // sibilance detail.
    let mut modified: Option<Vec<f32>> = None;
    // Simple deterministic PRNG. Seeded from channel_idx + frame
    // count (via an atomic counter per channel) so L and R produce
    // different noise patterns but each channel is reproducible
    // across runs.
    use std::sync::atomic::{AtomicU64, Ordering};
    static FRAME_COUNTER: [AtomicU64; 2] = [
        AtomicU64::new(0xC0FFEE11_u64),
        AtomicU64::new(0xDEADBEEF_u64),
    ];
    let ch = channel_idx.min(1);
    let mut rng_state = FRAME_COUNTER[ch].fetch_add(1, Ordering::Relaxed)
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);

    fn next_gauss(state: &mut u64) -> f32 {
        // xorshift64* → uniform in [0,1), then Box-Muller's simpler
        // cousin: sum-of-12 uniforms - 6 ≈ N(0,1). Good enough for
        // perceptual noise injection.
        let mut sum = -6.0_f32;
        for _ in 0..12 {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            sum += (*state as u32 as f32) * (1.0 / (u32::MAX as f32 + 1.0));
        }
        sum
    }

    let debug = std::env::var("FRANK_NOISE_DEBUG").is_ok();
    let mut hit = 0usize;
    let mut checked = 0usize;
    for b in 20..32 {
        if b >= ATRAC3_SUBBAND_TAB.len() - 1 {
            break;
        }
        checked += 1;
        let start = ATRAC3_SUBBAND_TAB[b];
        let end = ATRAC3_SUBBAND_TAB[b + 1];
        if end > coefficients.len() {
            break;
        }
        // Use peak/rms directly — our SFM-based tonality collapses to
        // 1.0 on quiet HF bands with wide coefficient variance, which
        // wrongly labels noise as tonal.
        let slice = &coefficients[start..end];
        let peak = slice.iter().map(|c| c.abs()).fold(0.0_f32, f32::max);
        let n = slice.len() as f32;
        let rms = (slice.iter().map(|c| c * c).sum::<f32>() / n).sqrt();
        if rms < 1e-10 {
            continue;  // silent band; nothing to inject
        }
        let signal_db = 20.0 * rms.log10();
        let ath_db = psycho.ath_db[b];
        if signal_db < ath_db - 6.0 {
            // Below hearing threshold — don't bother injecting
            // noise; the encoder will drop it anyway.
            continue;
        }
        let crest = peak / rms;
        if debug {
            eprintln!("[noise] b{:2}  crest={:.2}  rms={:.3e}  sig_db={:+.1}  ath={:+.1}",
                b, crest, rms, signal_db, ath_db);
        }
        // Tight noise-like test. Gaussian-noise crest depends on
        // sample count N: for N=32, E[crest] ≈ 2.4. Tonal content
        // easily reaches 4+. But wide bands with sine-on-noise can
        // also sit at crest 2.5-3. To avoid nuking real tonal
        // content we use a crest ≤ 2.3 cut AND restrict to very
        // lossy frames (low_pass source flagged). Belt-and-braces,
        // but v31 proved the looser version nuked music.
        if crest > 2.3 {
            continue;
        }
        if psycho.hf_energy_ratio > 0.001 {
            continue;  // real HF present in frame — keep the coefficients
        }
        hit += 1;

        // Replace the coefficients with Gaussian noise scaled to
        // exactly the same RMS.
        if modified.is_none() {
            modified = Some(coefficients.to_vec());
        }
        let out = modified.as_mut().unwrap();
        let mut noise_samples: Vec<f32> = (0..(end - start))
            .map(|_| next_gauss(&mut rng_state))
            .collect();
        let noise_rms = (noise_samples.iter().map(|c| c * c).sum::<f32>()
                         / noise_samples.len() as f32).sqrt().max(1e-10);
        let scale = rms / noise_rms;
        for (i, s) in noise_samples.iter_mut().enumerate() {
            *s *= scale;
            out[start + i] = *s;
        }
    }

    if debug {
        eprintln!("[noise] ch={} checked={} hit={}", ch, checked, hit);
    }
    match modified {
        Some(v) => std::borrow::Cow::Owned(v),
        None => std::borrow::Cow::Borrowed(coefficients),
    }
}
