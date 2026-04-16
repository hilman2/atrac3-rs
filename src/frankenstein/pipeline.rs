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
    _options: SearchOptions,
    target_bits: usize,
) -> Result<SpectrumEncoding> {
    // ATRAC3 is effectively always 44.1 kHz; keep it parameterised for
    // later tools.
    let sample_rate = 44100u32;

    let psycho = analysis::compute(coefficients, sample_rate);
    rdo::allocate(coefficients, coding_mode, target_bits, &psycho)
}
