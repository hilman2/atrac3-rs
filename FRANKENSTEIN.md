# Frankenstein — radical rewrite of the atrac3-rs encoder

**Status:** Plan. Runs parallel to the classic `atrac3/` module, switchable
through a CLI flag. Classic stays fully operational until Frankenstein
meets or exceeds it across the ears.py metric suite.

## Why rewrite

The current `atrac3/` pipeline has organic growth from ~14 iterations:
Psycho v2 baseline + eight Sony-tonmeister tricks + MIN_TBL floor + half a
dozen experiments (B/C/MAX_TBL/PromoWeight/TOL) documented across
CLAUDE.md and memory files. Each piece is defensible in isolation, but:

- Every new hook runs **lokal-greedy** against its own metric and steals
  bits from the others. There is no single global perceptual objective.
- The allocator has two post-promotion loops (sfIndex-desc + Robin Hood)
  both with their own priority function — they sometimes contradict each
  other.
- Scale-factor choice, initial tbl assignment, and promotion priority all
  carry separate, partly-overlapping heuristics (`effective_peak`,
  `tonal spread`, `LF_OVERSHOOT_WEIGHTS_Q8`, `promote_weight`, etc.).
- Experiments like option C proved the allocator is **not** globally
  optimal — a masking-aware promote pass can improve HF at the cost of
  catastrophic LF regression because there is no shared reservation
  between the passes.

Modern codecs (MP3 Model 2, AAC, Opus CELT) solve this with a single
**psychoacoustic driver** computed once per frame, which all allocation
stages consult. We will build that.

## Architecture: parallel module

```
atrac3-rs/src/
  atrac3/             # classic — unchanged
  frankenstein/
    analysis.rs       # Psycho-Drive (Bark + Spreading + ATH + tonality
                      # + transient detection + perceptual entropy)
    rdo.rs            # Lagrangian RDO allocator
    quantize.rs       # thin wrapper around the format's quant primitive
    gain.rs           # transient-aware gain envelope
    stereo.rs         # Mid/Side coupling decision + apply
    pipeline.rs       # Analysis → RDO → Quant → Gain → Bitstream
    mod.rs
  lib.rs              # re-exports both
```

**Shared with classic** (format-level, never duplicated):
- `container.rs`      RIFF/AT3 container
- `bitstream.rs`      bit-level writer
- `sound_unit.rs`     ATRAC3 Sound-Unit layout
- `qmf.rs`            QMF polyphase analysis
- `mdct.rs`           MDCT primitives
- Huffman tables (`ATRAC3_HUFF_TABS`, `ATRAC3_CLC_LENGTH_TAB`, ...)
- `quantize_subband` and `dequantized` (low-level coefficient quant)

**CLI switch:** `at3cmp proto-at3 --engine {classic,frankenstein}`.
Default `classic` during development; flip when FF passes all ears.py
pass/fail rollups on Crystallize + HateMe.

## Stage 1 — Psycho-Drive (cornerstone)

Single struct computed once per encoder frame, consumed by every later
stage. No policy — pure analysis.

```rust
struct PsychoDrive {
    /// Per-Bark-band signal energy in dB (24 bands).
    bark_energies:   [f32; 24],
    /// Spread masking curve projected back to ATRAC3 subbands (32),
    /// in dB. Each entry is the highest of own-band masking, spread
    /// from louder neighbours, and the absolute hearing threshold.
    subband_mask:    [f32; 32],
    /// Tonality (0 = flat noise, 1 = pure tone) per subband.
    /// Drives the signal-to-mask ratio: tonal → SMR 18 dB, noise → 6 dB.
    tonality:        [f32; 32],
    /// Absolute Threshold of Hearing per subband (from Terhardt).
    ath:             [f32; 32],
    /// Transient flag per QMF subband (4), with attack position in
    /// MDCT bins for the gain envelope.
    transients:      [Option<u16>; 4],
    /// Perceptual Entropy: bit-count needed to keep NMR ≤ 0 in all
    /// bands. Used to tell the allocator how tight/loose the budget is.
    pe_required_bits: f32,
    /// L-R correlation per subband — if > 0.85 and signal > ATH, the
    /// band is a candidate for Mid/Side intensity coupling.
    stereo_coupling_viable: [bool; 32],
}
```

Acceptance test: `psycho::compute(&coefficients, sr)` runs, returns
deterministic values, has a unit test locking in known inputs
(sine at 1 kHz, white noise, silence, transient click).

## Stage 2 — RDO allocator

Global bit allocation as Lagrangian-relaxed optimisation:

```
minimise   Σ_b  perceptual_weight(b) · distortion(b, tbl_b, sf_b)
s.t.       Σ_b  bits(b, tbl_b, sf_b) ≤ target_bits
```

Solved via λ-search (bisection):
1. Start with `λ_lo = 0, λ_hi = large`.
2. For each λ, for each band, pick the `(tbl, sf)` that minimises
   `bits + λ · perceptual_weight · distortion`.
3. Sum bits. If over budget, raise λ. If under, lower λ.
4. Converges in ~8-10 bisection steps.

**Perceptual weight** per band:
```
w(b) = max(0, signal_level_db(b) - subband_mask(b))   // above-mask first
     + tonality(b) · BONUS                             // tonal bands matter more
     - ATH_BELOW_HEADROOM(b)                           // below-ATH bands free
```

This replaces *all* of: `effective_peak/delta`, `promote_weight`,
`LF_OVERSHOOT_WEIGHTS_Q8`, `TONAL_SPREAD_Q0`, `MIN_TBL`.

The `(tbl, sf)` candidates per band are pre-computed with real
`quantize_subband` calls (we already do this in the classic path).

## Stage 3 — Transient-aware gain envelope

Calls `psycho.transients[qmf_band]` to decide gain-point count and
placement. Mirrors what Sony's tonmeister trick #8 was going for but is
informed by a real transient detector instead of heuristic thresholds.

## Stage 4 — Mid/Side intensity stereo

For each subband where `psycho.stereo_coupling_viable[b]` is true, encode
Mid signal only and a side-pan scalar. Saves ~30-50% of HF bits on
correlated music — those bits flow back into the RDO pool for Mid.

ATRAC3 format already supports a form of joint-stereo flag per frame; we
will use it.

## Stage 5 — Bitstream

Unchanged. Feeds the Sound-Unit writer from the classic path.

## Migration / Comparison Strategy

- Both engines coexist behind `--engine` flag.
- ears.py sweep runs `--engine classic` and `--engine frankenstein` on
  both Crystallize90 and HateMe, emits side-by-side HTML report.
- Frankenstein ships when its pass/fail rollup is **green or equal** on
  both samples, and no single metric regresses by more than 1 dB SNR
  relative to classic.
- Then classic is renamed to `legacy` and documented as deprecated; the
  Sony-trick folklore in CLAUDE.md points to Frankenstein as the
  reference implementation.

## What this deliberately does NOT do

- **No bitstream format change.** Every decision still produces a
  spec-compliant ATRAC3 frame.
- **No new metric.** We keep SNR / HF-Env-Corr / Pre-Echo / NMR /
  Loudness as acceptance criteria — the existing ears.py suite.
- **No port of Sony's SSE assembly.** The classic path has that
  knowledge encoded as data (`SONY_ENERGY_THRESHOLD`, etc.). We take the
  psychoacoustic *intent* of those tables forward, not the bit patterns.

## Phases / effort estimate

| Phase | Scope | Estimate |
|-------|-------|----------|
| 1 | `analysis.rs` — Psycho-Drive with unit tests | 1 session |
| 2 | `rdo.rs` — Lagrangian allocator wired to analysis | 1 session |
| 3 | `pipeline.rs` + CLI `--engine` switch, first full encode | 1 session |
| 4 | `gain.rs` transient-aware envelope | 0.5 session |
| 5 | `stereo.rs` Mid/Side coupling | 0.5 session |
| 6 | A/B parity pass — close the final gaps to classic | 1 session |
| 7 | Promote frankenstein to default, deprecate classic | short |

Total: ~5 focused sessions.

## Risks and mitigations

- **Sub-Bass SNR might drop from +5 dB above Sony to +1 dB.** The
  Lagrangian allocator is honest; Psycho v2 was biased toward LF. This
  will show up early in Phase 3. Mitigation: accept it — we have no
  evidence the +5 dB luxury is audible, and the bits free up other
  real problems.
- **Transient detection false positives** lead to gain-envelope over-use
  → could thin the transient. Mitigation: lock the threshold with unit
  tests against a transient-rich clip before wiring to the encoder.
- **RDO λ-search doesn't converge in pathological frames.** Mitigation:
  clamp to 10 iterations and fall back to classic's Phase 4 init for
  that frame.

## Naming

`frankenstein` because it's a *deliberately* cherry-picked hybrid from
MP3 (masking model), AAC (TNS principle + RDO), Opus (stereo coupling),
and the classic path (format + QMF/MDCT). Not a rewrite from first
principles — a composition of the best of each.
