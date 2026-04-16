//! Frankenstein encoder — a parallel rewrite of the atrac3-rs bit
//! allocator around a single psychoacoustic driver and a Lagrangian
//! rate-distortion optimisation.
//!
//! The goal is to replace the organically-grown local heuristics in
//! `crate::atrac3::quant` (Psycho v2 + eight Sony-tonmeister tricks +
//! MIN_TBL floor + various experiments) with:
//!
//!   1. A single per-frame psychoacoustic model (`analysis`) that every
//!      allocation decision consults.
//!   2. A Lagrangian RDO allocator (`rdo`) that picks global-optimal
//!      (tbl, sf) per band given that model.
//!   3. A thin `pipeline` that orchestrates analysis → rdo → quant →
//!      emit, with the same I/O shape as classic so A/B testing is
//!      zero-friction.
//!
//! The engine is selected at runtime through the `ATRAC3_ENGINE` env
//! var: `classic` (default) or `frankenstein`. Classic stays fully
//! operational until FRANKENSTEIN.md's ship criteria are met.

pub mod analysis;
pub mod pipeline;
pub mod rdo;

pub use pipeline::build_spectral_unit as build_spectral_unit_frankenstein;

pub fn engine_is_frankenstein() -> bool {
    std::env::var("ATRAC3_ENGINE")
        .map(|v| v.eq_ignore_ascii_case("frankenstein") || v == "1")
        .unwrap_or(false)
}
