use anyhow::Result;

use super::{
    quant::{ATRAC3_CLC_LENGTH_TAB, ATRAC3_INV_MAX_QUANT, encode_mantissas, scale_factor},
    quant_sony::{
        ATRAC3_SUBBAND_TAB, SONY_ENERGY_THRESHOLD, SONY_HIGH_BUDGET_ABS_THRESHOLD,
        SONY_LOW_BUDGET_SPACING_BIAS, SONY_TONAL_PROMINENCE_THRESHOLDS, SONY_TONAL_SEP_BIT_LEN,
        SonyHbEntry, sony_peak_to_sf_index_group, sony_score_to_tbl,
    },
    sound_unit::{
        CodingMode, RawBitPayload, TonalCell, TonalCodingModeSelector, TonalComponent, TonalEntry,
    },
};

/// Convert a Sony HB entry (u32 magnitude masked to SEP_BIT_LEN bits)
/// into our PendingTonalEntry with signed i8 mantissas. Sign-extension
/// unpacks the 2's-complement representation stored in the low N bits.
fn sony_hb_entry_to_pending(entry: &SonyHbEntry, tbl: u8) -> PendingTonalEntry {
    let bit_len = SONY_TONAL_SEP_BIT_LEN[tbl as usize];
    let mut mantissas_i8 = [0i8; 4];
    for (i, &m) in entry.mantissas.iter().enumerate() {
        if bit_len == 0 {
            mantissas_i8[i] = 0;
            continue;
        }
        let mask = (1u32 << bit_len) - 1;
        let m_masked = m & mask;
        let signed = if m_masked >= (1u32 << (bit_len - 1)) {
            (m_masked as i32) - (1i32 << bit_len)
        } else {
            m_masked as i32
        };
        mantissas_i8[i] = signed as i8;
    }
    PendingTonalEntry {
        absolute_position: entry.position as usize,
        scale_factor_index: entry.sf_index,
        mantissas: mantissas_i8,
    }
}

const LOW_BUDGET_QSTEP: u8 = 3;
const HIGH_BUDGET_QSTEP_LOW: u8 = 5;
const HIGH_BUDGET_QSTEP_HIGH: u8 = 7;
const HIGH_BUDGET_COMPONENT_SPLIT_SF: u8 = 30;
const HIGH_BUDGET_COMPONENT_COUNT: usize = 2;
const MAX_SONY_TONAL_ENTRIES: usize = 64;

#[derive(Debug, Clone)]
pub struct TonalExtractionResult {
    pub tonal_mode_selector: TonalCodingModeSelector,
    pub tonal_components: Vec<TonalComponent>,
    pub tonal_bits: usize,
    pub coded_qmf_bands: u8,
    pub tonal_subbands: [bool; 32],
    pub low_budget_path: bool,
}

impl TonalExtractionResult {
    pub fn empty(coded_qmf_bands: u8) -> Self {
        empty_tonal_result(coded_qmf_bands, false)
    }
}

#[derive(Clone)]
struct PendingTonalEntry {
    absolute_position: usize,
    scale_factor_index: u8,
    mantissas: [i8; 4],
}

impl PendingTonalEntry {
    fn qmf_band(&self) -> usize {
        self.absolute_position >> 8
    }

    fn cell_index(&self) -> usize {
        self.absolute_position >> 6
    }

    fn payload(
        &self,
        coding_mode: CodingMode,
        qstep: u8,
        coded_values_minus_one: u8,
    ) -> Result<RawBitPayload> {
        encode_mantissas(
            qstep,
            coding_mode,
            &self.mantissas[..=coded_values_minus_one as usize],
        )
    }

    fn total_bits(
        &self,
        coding_mode: CodingMode,
        qstep: u8,
        coded_values_minus_one: u8,
    ) -> Result<usize> {
        Ok(12
            + self
                .payload(coding_mode, qstep, coded_values_minus_one)?
                .bit_len())
    }

    fn apply_to_residual(&self, residual: &mut [f32], qstep: u8) {
        let scale = scale_factor(self.scale_factor_index) * ATRAC3_INV_MAX_QUANT[qstep as usize];
        for (index, &mantissa) in self.mantissas.iter().enumerate() {
            if let Some(sample) = residual.get_mut(self.absolute_position + index) {
                *sample -= mantissa as f32 * scale;
            }
        }
    }

    fn restore_to_residual(&self, residual: &mut [f32], qstep: u8) {
        let scale = scale_factor(self.scale_factor_index) * ATRAC3_INV_MAX_QUANT[qstep as usize];
        for (index, &mantissa) in self.mantissas.iter().enumerate() {
            if let Some(sample) = residual.get_mut(self.absolute_position + index) {
                *sample += mantissa as f32 * scale;
            }
        }
    }

    fn to_tonal_entry(
        &self,
        coding_mode: CodingMode,
        qstep: u8,
        coded_values_minus_one: u8,
    ) -> Result<TonalEntry> {
        Ok(TonalEntry {
            scale_factor_index: self.scale_factor_index,
            position: (self.absolute_position & 0x3f) as u8,
            payload: self.payload(coding_mode, qstep, coded_values_minus_one)?,
        })
    }
}

#[derive(Clone)]
struct PendingLowBudgetLayout {
    components: Vec<Vec<PendingTonalEntry>>,
    coded_values_minus_one: u8,
}

#[derive(Clone)]
struct GroupedPendingComponent {
    band_flags: Vec<bool>,
    cells: Vec<Vec<PendingTonalEntry>>,
}

#[derive(Clone)]
struct PendingTonalComponent {
    qstep: u8,
    band_flags: Vec<bool>,
    cells: Vec<Vec<PendingTonalEntry>>,
}

impl PendingTonalComponent {
    fn new(qmf_bands: usize, qstep: u8) -> Self {
        Self {
            qstep,
            band_flags: vec![false; qmf_bands],
            cells: vec![Vec::new(); qmf_bands * 4],
        }
    }
}

pub fn extract_tonal_components(
    residual: &mut [f32],
    budget_bits: usize,
    coded_qmf_bands: u8,
    _coding_mode: CodingMode,
    max_entries: usize,
    is_leader: bool,
    initial_band_limit: u8,
) -> Result<TonalExtractionResult> {
    let qmf_bands = coded_qmf_bands as usize;
    let total_cells = qmf_bands * 4;
    let spectral_end = (qmf_bands * 256).min(residual.len());
    let base_bits = 5 + 2 + qmf_bands + 3 + 3;
    let low_budget_bits = budget_bits / 4;
    let low_budget = tonal_hot_group_count(&residual[..spectral_end]) * 32 >= budget_bits;

    let _ = is_leader;
    if low_budget && low_budget_bits < base_bits + 24 {
        return Ok(empty_tonal_result(coded_qmf_bands, true));
    }

    // At 132 kbps ATRAC3 VLC stereo there is no joint-stereo / matrix
    // encoding — both channels are coded independently and must share the
    // same tonal pipeline. Historic `is_leader` gating starved the right
    // channel of tonal extraction and produced near-silent output.
    if low_budget {
        // 1:1 Sony-Port FUN_00438ed0 ist Default (Iter #3.5). Wenn der
        // Workbench leere Entries liefert, geben wir ein leeres Ergebnis
        // zurück statt in eine Heuristik zu fallen.
        let _ = total_cells;
        let _ = low_budget_bits;
        if let Some(result) = extract_low_budget_tonal_components_sony(
            residual,
            spectral_end,
            qmf_bands,
            budget_bits as i32,
            base_bits as i32,
            max_entries,
            is_leader,
            initial_band_limit,
        )? {
            return Ok(result);
        }
        return Ok(empty_tonal_result(coded_qmf_bands, true));
    }

    // 1:1 FUN_00438830 Port (SonyHbWorkbench) ist jetzt einziger HB-Pfad.
    // Iter #3.5: ATRAC_RUST_HB- und ATRAC_FORCE_HB-Gates entfernt, Heuristik
    // `extract_high_budget_tonal_components` ist tot.
    let _ = total_cells;
    extract_high_budget_sony_exact(
        residual,
        spectral_end,
        qmf_bands,
        total_cells,
        budget_bits.saturating_sub(200),
        max_entries,
        coded_qmf_bands,
    )
}

/// Bit-exact Sony FUN_00438830 extractor. Builds the high-budget tonal
/// layout via `SonyHbWorkbench`, then converts the SonyHbEntry pool into
/// our PendingTonalEntry / TonalComponent format for the bitstream.
fn extract_high_budget_sony_exact(
    residual: &mut [f32],
    spectral_end: usize,
    qmf_bands: usize,
    _total_cells: usize,
    tonal_budget: usize,
    max_entries: usize,
    coded_qmf_bands: u8,
) -> Result<TonalExtractionResult> {
    use super::quant_sony::SonyHbWorkbench;

    let entry_limit = max_entries.min(MAX_SONY_TONAL_ENTRIES);
    if entry_limit == 0 {
        return Ok(empty_tonal_result(coded_qmf_bands, false));
    }

    // Build workbench: residual is the spectrum window Sony sees. Pad to
    // 1024 if spectral_end is smaller (the allocator sees 1024 samples;
    // Sony writes beyond spectral_end only for band-peak recompute which
    // is guarded by local_514).
    let local_514 = coded_qmf_bands_to_local_514(coded_qmf_bands, spectral_end);
    let mut residual_owned = residual.to_vec();
    if residual_owned.len() < 1024 {
        residual_owned.resize(1024, 0.0);
    }

    let mut workbench = SonyHbWorkbench::new(residual_owned, local_514);
    let mut au_504 = vec![0u32; 33];
    let mut local_400 = vec![0u32; 256];
    let mut prev_coded_qmf_bands: u32 = coded_qmf_bands as u32;

    let total_bits = workbench.build_high_budget_tonals(
        tonal_budget as i32,
        &mut au_504,
        &mut local_400,
        &mut prev_coded_qmf_bands,
    );

    // Copy modified residual back to caller's slice.
    let copy_len = spectral_end
        .min(workbench.residual.len())
        .min(residual.len());
    residual[..copy_len].copy_from_slice(&workbench.residual[..copy_len]);

    if workbench.entries.is_empty() {
        return Ok(empty_tonal_result(coded_qmf_bands, false));
    }

    // Convert Sony channels → PendingTonalComponent pair.
    let mut components = [
        PendingTonalComponent::new(qmf_bands, HIGH_BUDGET_QSTEP_LOW),
        PendingTonalComponent::new(qmf_bands, HIGH_BUDGET_QSTEP_HIGH),
    ];
    for (ch_idx, channel) in workbench.channels.iter().enumerate() {
        for (cell_idx, cell) in channel.cells.iter().enumerate() {
            for slot in 0..(cell.count as usize) {
                let entry_idx = cell.entry_indices[slot] as usize;
                if entry_idx >= workbench.entries.len() {
                    continue;
                }
                let pending = sony_hb_entry_to_pending(&workbench.entries[entry_idx], channel.tbl);
                let qmf_band = pending.qmf_band();
                if qmf_band >= qmf_bands {
                    continue;
                }
                if cell_idx < components[ch_idx].cells.len() {
                    components[ch_idx].cells[cell_idx].push(pending);
                    components[ch_idx].band_flags[qmf_band] = true;
                }
            }
        }
    }

    let tonal_components = components
        .into_iter()
        .map(|component| {
            pending_cells_to_tonal_component(
                component.band_flags,
                component.cells,
                CodingMode::Clc,
                component.qstep,
                3,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let tonal_subbands = tonal_subbands_from_components(&tonal_components);
    let tonal_bits = tonal_bit_cost(&tonal_components, TonalCodingModeSelector::AllClc);
    let _ = total_bits;

    Ok(TonalExtractionResult {
        tonal_mode_selector: TonalCodingModeSelector::AllClc,
        tonal_components,
        tonal_bits,
        coded_qmf_bands,
        tonal_subbands,
        low_budget_path: false,
    })
}

fn coded_qmf_bands_to_local_514(coded_qmf_bands: u8, spectral_end: usize) -> usize {
    let _ = coded_qmf_bands;
    // Compute local_514 from spectral_end: find the largest subband whose
    // end-offset is <= spectral_end.
    for band in (0..32).rev() {
        if ATRAC3_SUBBAND_TAB[band + 1] <= spectral_end {
            return band + 1;
        }
    }
    1
}

fn tonal_hot_group_count(coefficients: &[f32]) -> usize {
    coefficients
        .chunks(4)
        .filter(|chunk| sony_peak_to_sf_index_group(chunk) > 7)
        .count()
}

fn extract_low_budget_tonal_components(
    residual: &mut [f32],
    spectral_end: usize,
    qmf_bands: usize,
    total_cells: usize,
    budget_bits: usize,
    tonal_budget: usize,
    base_bits: usize,
    max_entries: usize,
) -> Result<TonalExtractionResult> {
    let (subband_count, band_peaks, tbl_indices, _band_scores) =
        low_budget_band_state(&residual[..spectral_end], spectral_end, budget_bits);

    let new_band_bits = 12usize;
    let mut cells: Vec<Vec<PendingTonalEntry>> = vec![Vec::new(); total_cells];
    let mut band_active = vec![false; qmf_bands];
    let mut total_bits_clc = base_bits;
    let mut total_bits_vlc = base_bits;
    let mut vlc_possible = true;
    let mut total_entries = 0usize;
    let mut tonal_subbands = [false; 32];
    let mut previous_band_last_position: Option<usize> = None;

    for band in 0..subband_count {
        if total_entries >= max_entries {
            break;
        }

        let tbl_index = tbl_indices[band];
        if tbl_index == 0 {
            continue;
        }

        let band_start = ATRAC3_SUBBAND_TAB[band];
        let band_end = ATRAC3_SUBBAND_TAB[band + 1];
        let band_width = band_end - band_start;
        let threshold = low_budget_abs_threshold(band, band_peaks[band], tbl_index);
        let max_candidates = (band_width + 8) >> 4;
        let scan_start = previous_band_last_position
            .map(|position| (position + 4).max(band_start))
            .unwrap_or(band_start);
        let Some(candidates) = scan_low_budget_positions(
            &residual[..spectral_end],
            scan_start,
            band_end,
            max_candidates,
            threshold,
        ) else {
            continue;
        };

        let mut band_last_position = previous_band_last_position;
        for mut position in candidates {
            if total_entries >= max_entries {
                break;
            }

            position =
                backtrack_contiguous_peak(&residual[..spectral_end], band_last_position, position);

            let Some(entry) =
                quantize_low_budget_entry(&residual[..spectral_end], position, band_end)?
            else {
                continue;
            };

            let qmf_band = entry.qmf_band();
            let cell_index = entry.cell_index();
            if qmf_band >= qmf_bands || cell_index >= total_cells || cells[cell_index].len() >= 7 {
                continue;
            }

            let band_cost = if band_active[qmf_band] {
                0
            } else {
                new_band_bits
            };
            let entry_bits_clc = entry.total_bits(CodingMode::Clc, LOW_BUDGET_QSTEP, 3)?;
            let next_bits_vlc = if vlc_possible {
                match entry.total_bits(CodingMode::Vlc, LOW_BUDGET_QSTEP, 3) {
                    Ok(bits) => total_bits_vlc + band_cost + bits,
                    Err(_) => usize::MAX,
                }
            } else {
                usize::MAX
            };
            let next_bits_clc = total_bits_clc + band_cost + entry_bits_clc;

            if next_bits_clc.min(next_bits_vlc) > tonal_budget {
                continue;
            }

            entry.apply_to_residual(residual, LOW_BUDGET_QSTEP);
            cells[cell_index].push(entry);
            if !band_active[qmf_band] {
                band_active[qmf_band] = true;
            }

            total_bits_clc = next_bits_clc;
            total_bits_vlc = next_bits_vlc;
            vlc_possible &= next_bits_vlc != usize::MAX;
            total_entries += 1;
            band_last_position = cells[cell_index]
                .last()
                .map(|pending| pending.absolute_position);

            if let Some(subband) = subband_index_for_position(
                cells[cell_index]
                    .last()
                    .map(|pending| pending.absolute_position)
                    .unwrap_or(position),
            ) {
                tonal_subbands[subband] = true;
            }
        }

        previous_band_last_position = band_last_position;
    }

    if total_entries == 0 {
        return Ok(empty_tonal_result(qmf_bands as u8, true));
    }

    let mut flat_entries: Vec<PendingTonalEntry> = cells.into_iter().flatten().collect();
    flat_entries.sort_by_key(|entry| entry.absolute_position);

    if flat_entries.len() == 1 && (flat_entries[0].absolute_position < 0x80 || budget_bits < 0x44c)
    {
        return Ok(empty_tonal_result(qmf_bands as u8, true));
    }

    let base_layout = PendingLowBudgetLayout {
        components: vec![flat_entries],
        coded_values_minus_one: 3,
    };
    let mut final_layout = base_layout.clone();
    let tail_zero_count = final_layout.components[0]
        .iter()
        .filter(|entry| entry.mantissas[3] == 0)
        .count();
    let all_tail_zero = tail_zero_count == final_layout.components[0].len();

    if all_tail_zero {
        reduce_low_budget_layout(&mut final_layout);
    } else if let Some(mut split_layout) = split_low_budget_layout(&final_layout, qmf_bands) {
        reduce_low_budget_layout(&mut split_layout);
        if best_low_budget_layout_cost(&split_layout, qmf_bands, base_bits)?
            < best_low_budget_layout_cost(&base_layout, qmf_bands, base_bits)?
        {
            final_layout = split_layout;
        }
    }

    let clc_cost = low_budget_layout_cost(&final_layout, qmf_bands, base_bits, CodingMode::Clc)?;
    let vlc_cost = low_budget_layout_cost(&final_layout, qmf_bands, base_bits, CodingMode::Vlc)?;

    let use_vlc = vlc_possible && vlc_cost < clc_cost;
    let coding_mode = if use_vlc {
        CodingMode::Vlc
    } else {
        CodingMode::Clc
    };
    let grouped_components = final_layout
        .components
        .iter()
        .map(|entries| group_pending_component(entries, qmf_bands))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| anyhow::anyhow!("low-budget tonal layout overflowed cell or qmf bounds"))?;
    let tonal_components = grouped_components
        .into_iter()
        .map(|component| {
            pending_component_to_tonal_component(
                component,
                coding_mode,
                LOW_BUDGET_QSTEP,
                final_layout.coded_values_minus_one,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(TonalExtractionResult {
        tonal_mode_selector: if use_vlc {
            TonalCodingModeSelector::AllVlc
        } else {
            TonalCodingModeSelector::AllClc
        },
        tonal_components,
        tonal_bits: if use_vlc { vlc_cost } else { clc_cost },
        coded_qmf_bands: qmf_bands as u8,
        tonal_subbands,
        low_budget_path: true,
    })
}

/// Build the low-budget workbench context (Phase-4 band score,
/// Phase-4 tbl flag, Phase-1 band peak, band_count, base_bits) from the
/// residual window and drive the 1:1 Sony port `sony_tonal_build_low_budget`.
///
/// Converts the resulting entry pool back to our PendingTonalEntry
/// representation and produces a `TonalExtractionResult`. Returns `None`
/// if the workbench produced zero entries (caller should fall back to
/// the heuristic path for now).
fn extract_low_budget_tonal_components_sony(
    residual: &mut [f32],
    spectral_end: usize,
    qmf_bands: usize,
    budget_bits: i32,
    base_bits: i32,
    max_entries: usize,
    is_leader: bool,
    initial_band_limit: u8,
) -> Result<Option<TonalExtractionResult>> {
    use super::quant_sony::{
        SonyLbWorkbench, sony_phase1_lb_inputs, sony_tonal_build_low_budget,
    };

    // Spectrum: pad to 1024 so Sony's workbench has room for
    // post-processing windows.
    let mut spectrum_owned = residual.to_vec();
    if spectrum_owned.len() < 1024 {
        spectrum_owned.resize(1024, 0.0);
    }

    // 1:1 port of Sony's caller-side Phase-1 + Phase-4 (FUN_00437bb0
    // Ghidra 42322-42400). Produces the exact `band_count` (= local_514),
    // `band_peak`, `band_state_low`, `band_score`, and `base_bits`
    // arrays FUN_00438ed0 reads. Previously this used the Rust-side
    // `low_budget_band_state` proxy which diverged from Sony in the
    // local_514 extension logic and the bvar1 threshold.
    let phase1 = sony_phase1_lb_inputs(
        &spectrum_owned,
        budget_bits,
        !is_leader,
        initial_band_limit,
    );

    let mut workbench = SonyLbWorkbench::new_from_phase1(spectrum_owned, &phase1);

    let _total_bits = sony_tonal_build_low_budget(budget_bits, &mut workbench);

    // Copy modified residual back.
    let copy_len = spectral_end
        .min(workbench.spectrum.len())
        .min(residual.len());
    residual[..copy_len].copy_from_slice(&workbench.spectrum[..copy_len]);

    if workbench.entries.is_empty() {
        return Ok(None);
    }

    // Convert entries → PendingTonalEntry list. Sony encodes mantissas
    // with the tbl's sign-extended bit width; for LB path tbl=3 so
    // SONY_TONAL_SEP_BIT_LEN[3] = 3 bits.
    let sep_len = super::quant_sony::SONY_TONAL_SEP_BIT_LEN[workbench.tbl as usize];
    let mut tonal_subbands = [false; 32];
    let mut flat_entries: Vec<PendingTonalEntry> = Vec::with_capacity(workbench.entries.len());

    for ent in workbench.entries.iter().take(max_entries) {
        let mut mantissas_i8 = [0i8; 4];
        for i in 0..4 {
            let m = ent.mantissas[i];
            if sep_len == 0 {
                mantissas_i8[i] = 0;
                continue;
            }
            let mask = (1u32 << sep_len) - 1;
            let m_masked = m & mask;
            let signed = if m_masked >= (1u32 << (sep_len - 1)) {
                (m_masked as i32) - (1i32 << sep_len)
            } else {
                m_masked as i32
            };
            mantissas_i8[i] = signed as i8;
        }
        flat_entries.push(PendingTonalEntry {
            absolute_position: ent.position as usize,
            scale_factor_index: ent.sf_index as u8,
            mantissas: mantissas_i8,
        });
        if let Some(subband) = subband_index_for_position(ent.position as usize) {
            tonal_subbands[subband] = true;
        }
    }

    flat_entries.sort_by_key(|entry| entry.absolute_position);

    if flat_entries.is_empty() {
        return Ok(None);
    }

    // Build single-component layout and convert.
    let layout = PendingLowBudgetLayout {
        components: vec![flat_entries],
        coded_values_minus_one: workbench.count_minus_one.clamp(0, 3) as u8,
    };

    let clc_cost = low_budget_layout_cost(&layout, qmf_bands, base_bits as usize, CodingMode::Clc)?;
    let vlc_cost =
        low_budget_layout_cost(&layout, qmf_bands, base_bits as usize, CodingMode::Vlc).ok();

    let use_vlc = matches!(vlc_cost, Some(v) if v < clc_cost);
    let coding_mode = if use_vlc {
        CodingMode::Vlc
    } else {
        CodingMode::Clc
    };

    let grouped_components = layout
        .components
        .iter()
        .map(|entries| group_pending_component(entries, qmf_bands))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| anyhow::anyhow!("sony-lb layout overflowed cell/qmf bounds"))?;
    let tonal_components = grouped_components
        .into_iter()
        .map(|component| {
            pending_component_to_tonal_component(
                component,
                coding_mode,
                LOW_BUDGET_QSTEP,
                layout.coded_values_minus_one,
            )
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(TonalExtractionResult {
        tonal_mode_selector: if use_vlc {
            TonalCodingModeSelector::AllVlc
        } else {
            TonalCodingModeSelector::AllClc
        },
        tonal_components,
        tonal_bits: if use_vlc {
            vlc_cost.unwrap_or(clc_cost)
        } else {
            clc_cost
        },
        coded_qmf_bands: qmf_bands as u8,
        tonal_subbands,
        low_budget_path: true,
    }))
}

fn split_low_budget_layout(
    layout: &PendingLowBudgetLayout,
    qmf_bands: usize,
) -> Option<PendingLowBudgetLayout> {
    if layout.components.len() != 1 {
        return None;
    }

    let source_band_flags = group_pending_component(&layout.components[0], qmf_bands)?.band_flags;
    let mut first = Vec::with_capacity(layout.components[0].len());
    let mut second = Vec::new();

    for entry in &layout.components[0] {
        let mut first_entry = entry.clone();
        if entry.mantissas[3] != 0 {
            let new_position = entry.absolute_position + 2;
            let target_qmf_band = new_position >> 8;
            if new_position > 0x3fd
                || target_qmf_band >= qmf_bands
                || !source_band_flags[target_qmf_band]
            {
                return None;
            }
            first_entry.mantissas[2] = 0;
            first_entry.mantissas[3] = 0;
            second.push(PendingTonalEntry {
                absolute_position: new_position,
                scale_factor_index: entry.scale_factor_index,
                mantissas: [entry.mantissas[2], entry.mantissas[3], 0, 0],
            });
        }
        first.push(first_entry);
    }

    if first.len() + second.len() > 64 {
        return None;
    }

    let split_layout = PendingLowBudgetLayout {
        components: vec![first, second],
        coded_values_minus_one: layout.coded_values_minus_one,
    };

    split_layout
        .components
        .iter()
        .all(|entries| group_pending_component(entries, qmf_bands).is_some())
        .then_some(split_layout)
}

fn reduce_low_budget_layout(layout: &mut PendingLowBudgetLayout) {
    while layout.coded_values_minus_one > 0 {
        let tail_index = layout.coded_values_minus_one as usize;
        let mut should_stop = false;

        for component_entries in &mut layout.components {
            for entry in component_entries.iter_mut().rev() {
                if entry.mantissas[tail_index] != 0 {
                    entry.absolute_position += 1;
                    for index in 0..tail_index {
                        entry.mantissas[index] = entry.mantissas[index + 1];
                    }
                }
            }
        }

        layout.coded_values_minus_one -= 1;
        let new_tail_index = layout.coded_values_minus_one as usize;
        for component_entries in &layout.components {
            for entry in component_entries {
                if entry.mantissas[new_tail_index] != 0
                    && (entry.mantissas[0] != 0 || (entry.absolute_position & 0x3f) == 0x3f)
                {
                    should_stop = true;
                    break;
                }
            }
            if should_stop {
                break;
            }
        }

        if should_stop {
            break;
        }
    }
}

fn best_low_budget_layout_cost(
    layout: &PendingLowBudgetLayout,
    qmf_bands: usize,
    base_bits: usize,
) -> Result<usize> {
    Ok(
        low_budget_layout_cost(layout, qmf_bands, base_bits, CodingMode::Clc)?.min(
            low_budget_layout_cost(layout, qmf_bands, base_bits, CodingMode::Vlc)?,
        ),
    )
}

fn low_budget_layout_cost(
    layout: &PendingLowBudgetLayout,
    qmf_bands: usize,
    base_bits: usize,
    coding_mode: CodingMode,
) -> Result<usize> {
    let mut total_bits =
        base_bits + (layout.components.len().saturating_sub(1)) * (qmf_bands + 3 + 3);

    for component_entries in &layout.components {
        let grouped = group_pending_component(component_entries, qmf_bands)
            .ok_or_else(|| anyhow::anyhow!("invalid low-budget tonal component layout"))?;
        total_bits += grouped.band_flags.iter().filter(|&&flag| flag).count() * 12;
        for cell in grouped.cells {
            for entry in cell {
                total_bits += entry.total_bits(
                    coding_mode,
                    LOW_BUDGET_QSTEP,
                    layout.coded_values_minus_one,
                )?;
            }
        }
    }

    Ok(total_bits)
}

fn group_pending_component(
    entries: &[PendingTonalEntry],
    qmf_bands: usize,
) -> Option<GroupedPendingComponent> {
    let mut band_flags = vec![false; qmf_bands];
    let mut cells = vec![Vec::new(); qmf_bands * 4];

    for entry in entries.iter().cloned() {
        let qmf_band = entry.absolute_position >> 8;
        let cell_index = entry.absolute_position >> 6;
        if qmf_band >= qmf_bands || cell_index >= cells.len() || entry.absolute_position > 0x3fc {
            return None;
        }
        if cells[cell_index].len() >= 7 {
            return None;
        }
        band_flags[qmf_band] = true;
        cells[cell_index].push(entry);
    }

    Some(GroupedPendingComponent { band_flags, cells })
}

fn pending_component_to_tonal_component(
    component: GroupedPendingComponent,
    coding_mode: CodingMode,
    qstep: u8,
    coded_values_minus_one: u8,
) -> Result<TonalComponent> {
    let cells = component
        .cells
        .into_iter()
        .map(|entries| {
            entries
                .into_iter()
                .map(|entry| entry.to_tonal_entry(coding_mode, qstep, coded_values_minus_one))
                .collect::<Result<Vec<_>>>()
                .map(|entries| TonalCell { entries })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(TonalComponent {
        band_flags: component.band_flags,
        coded_values_minus_one,
        quant_step_index: qstep,
        coding_mode: None,
        cells,
    })
}

fn pending_cells_to_tonal_component(
    band_flags: Vec<bool>,
    cells: Vec<Vec<PendingTonalEntry>>,
    coding_mode: CodingMode,
    qstep: u8,
    coded_values_minus_one: u8,
) -> Result<TonalComponent> {
    let cells = cells
        .into_iter()
        .map(|entries| {
            entries
                .into_iter()
                .map(|entry| entry.to_tonal_entry(coding_mode, qstep, coded_values_minus_one))
                .collect::<Result<Vec<_>>>()
                .map(|entries| TonalCell { entries })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(TonalComponent {
        band_flags,
        coded_values_minus_one,
        quant_step_index: qstep,
        coding_mode: None,
        cells,
    })
}

fn tonal_bit_cost(components: &[TonalComponent], selector: TonalCodingModeSelector) -> usize {
    let mut total_bits = 5usize;
    if components.is_empty() {
        return total_bits;
    }

    total_bits += 2;
    for component in components {
        total_bits += component.band_flags.len() + 6;
        if selector == TonalCodingModeSelector::PerComponent {
            total_bits += 1;
        }
        for (cell_index, cell) in component.cells.iter().enumerate() {
            if !component.band_flags[cell_index / 4] {
                continue;
            }
            total_bits += 3;
            for entry in &cell.entries {
                total_bits += 12 + entry.payload.bit_len();
            }
        }
    }

    total_bits
}

fn tonal_subbands_from_components(components: &[TonalComponent]) -> [bool; 32] {
    let mut tonal_subbands = [false; 32];
    for component in components {
        for (cell_index, cell) in component.cells.iter().enumerate() {
            if !component.band_flags[cell_index / 4] {
                continue;
            }
            let cell_start = cell_index << 6;
            for entry in &cell.entries {
                let absolute_position = cell_start + entry.position as usize;
                if let Some(subband) = subband_index_for_position(absolute_position) {
                    tonal_subbands[subband] = true;
                }
            }
        }
    }
    tonal_subbands
}

fn high_budget_component_index(peak_sf: u8) -> usize {
    usize::from(peak_sf > HIGH_BUDGET_COMPONENT_SPLIT_SF)
}

fn weakest_replaceable_high_budget_entry_index(
    component: &PendingTonalComponent,
    cell_index: usize,
    candidate_sf: u8,
) -> Option<usize> {
    component.cells[cell_index]
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.scale_factor_index < candidate_sf)
        .min_by_key(|(_, entry)| entry.scale_factor_index)
        .map(|(index, _)| index)
}

fn extract_high_budget_tonal_components(
    residual: &mut [f32],
    spectral_end: usize,
    qmf_bands: usize,
    total_cells: usize,
    tonal_budget: usize,
    max_entries: usize,
    coded_qmf_bands: u8,
) -> Result<TonalExtractionResult> {
    let entry_limit = max_entries.min(MAX_SONY_TONAL_ENTRIES);
    let base_bits = 5 + 2 + HIGH_BUDGET_COMPONENT_COUNT * (qmf_bands + 6);

    if entry_limit == 0 || tonal_budget < base_bits + 12 + 28 {
        return Ok(empty_tonal_result(coded_qmf_bands, false));
    }

    let mut total_bits = base_bits;
    let mut total_entries = 0usize;
    let mut components = [
        PendingTonalComponent::new(qmf_bands, HIGH_BUDGET_QSTEP_LOW),
        PendingTonalComponent::new(qmf_bands, HIGH_BUDGET_QSTEP_HIGH),
    ];

    loop {
        let entries_before_pass = total_entries;
        let mut position = 0usize;

        while position < spectral_end && total_entries < entry_limit {
            if residual[position].abs() < SONY_HIGH_BUDGET_ABS_THRESHOLD {
                position += 1;
                continue;
            }

            let absolute_position = position.min(0x3fc);
            let group_end = (absolute_position + 4).min(spectral_end);
            let peak_sf = sony_peak_to_sf_index_group(&residual[absolute_position..group_end]);
            let preferred = high_budget_component_index(peak_sf);
            let cell_index = absolute_position >> 6;
            if cell_index >= total_cells {
                break;
            }

            let mut target = preferred;
            let mut expands_layout = true;
            let mut replaced_entry: Option<(usize, PendingTonalEntry)> = None;

            if components[target].cells[cell_index].len() >= 7 {
                let spill = 1 - target;
                if components[spill].cells[cell_index].len() < 7 {
                    target = spill;
                } else if let Some(index) = weakest_replaceable_high_budget_entry_index(
                    &components[preferred],
                    cell_index,
                    peak_sf,
                ) {
                    let removed = components[preferred].cells[cell_index].remove(index);
                    removed.restore_to_residual(residual, components[preferred].qstep);
                    replaced_entry = Some((index, removed));
                    expands_layout = false;
                } else {
                    position += 1;
                    continue;
                }
            }

            let qstep = components[target].qstep;
            let entry = quantize_fixed_clc_entry(
                &residual[..spectral_end],
                absolute_position,
                spectral_end,
                qstep,
            )?;
            let Some(entry) = entry else {
                if let Some((index, removed)) = replaced_entry.take() {
                    removed.apply_to_residual(residual, components[target].qstep);
                    components[target].cells[cell_index].insert(index, removed);
                }
                position += 1;
                continue;
            };

            let qmf_band = entry.qmf_band();
            if qmf_band >= qmf_bands {
                if let Some((index, removed)) = replaced_entry.take() {
                    removed.apply_to_residual(residual, components[target].qstep);
                    components[target].cells[cell_index].insert(index, removed);
                }
                position += 1;
                continue;
            }

            let band_cost = if expands_layout && !components[target].band_flags[qmf_band] {
                12
            } else {
                0
            };
            let entry_bits = if expands_layout {
                entry.total_bits(CodingMode::Clc, qstep, 3)?
            } else {
                0
            };

            if expands_layout && total_bits + band_cost + entry_bits > tonal_budget {
                if let Some((index, removed)) = replaced_entry.take() {
                    removed.apply_to_residual(residual, components[target].qstep);
                    components[target].cells[cell_index].insert(index, removed);
                }
                position += 1;
                continue;
            }

            entry.apply_to_residual(residual, qstep);
            components[target].cells[cell_index].push(entry);
            components[target].band_flags[qmf_band] = true;

            if expands_layout {
                total_entries += 1;
                total_bits += band_cost + entry_bits;
            }

            position += 4;
        }

        if total_entries == entries_before_pass || total_entries >= entry_limit {
            break;
        }
    }

    if total_entries == 0 {
        return Ok(empty_tonal_result(coded_qmf_bands, false));
    }

    let tonal_components = components
        .into_iter()
        .map(|component| {
            pending_cells_to_tonal_component(
                component.band_flags,
                component.cells,
                CodingMode::Clc,
                component.qstep,
                3,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let tonal_subbands = tonal_subbands_from_components(&tonal_components);
    let tonal_bits = tonal_bit_cost(&tonal_components, TonalCodingModeSelector::AllClc);

    Ok(TonalExtractionResult {
        tonal_mode_selector: TonalCodingModeSelector::AllClc,
        tonal_components,
        tonal_bits,
        coded_qmf_bands,
        tonal_subbands,
        low_budget_path: false,
    })
}

fn low_budget_band_state(
    residual: &[f32],
    spectral_end: usize,
    budget_bits: usize,
) -> (usize, [u8; 32], [u8; 32], [i32; 32]) {
    let subband_count = coded_subband_count(spectral_end);
    let mut band_peaks = [0u8; 32];
    let mut band_active_groups = [0u32; 33];
    let mut band_scores = [0i32; 32];
    let mut total_sum = 0u32;
    let mut global_peak = 0u32;
    let mut active_group_count = 0u32;

    for band in 0..subband_count {
        let start = ATRAC3_SUBBAND_TAB[band];
        let end = ATRAC3_SUBBAND_TAB[band + 1];
        let mut band_sum = 0u32;
        let mut band_peak = 0u32;

        for chunk in residual[start..end].chunks(4) {
            let group_sf = sony_peak_to_sf_index_group(chunk) as u32;
            band_sum += group_sf;
            band_peak = band_peak.max(group_sf);
            if group_sf > 7 {
                active_group_count += 1;
            }
        }

        if band_peak > global_peak {
            global_peak = band_peak;
        } else if (band_sum as i32) < SONY_ENERGY_THRESHOLD[band] || band_peak < 3 {
            band_sum = 0;
            band_peak = 0;
        }

        band_peaks[band] = band_peak as u8;
        band_active_groups[band + 1] = active_group_count;
        total_sum += band_sum;
    }

    let budget = budget_bits as u32;
    let bvar1 = if budget.saturating_mul(2) < band_active_groups[subband_count].saturating_mul(12) {
        11
    } else {
        10
    };

    let mut tbl_indices = [0u8; 32];
    for band in 0..subband_count {
        let peak = band_peaks[band];
        if peak > 2 {
            let score = i32::from(peak) * 256 - total_sum as i32;
            band_scores[band] = score;
            tbl_indices[band] = sony_score_to_tbl(score, bvar1, 1) as u8;
        }
    }

    (subband_count, band_peaks, tbl_indices, band_scores)
}

fn coded_subband_count(spectral_end: usize) -> usize {
    (0..32)
        .take_while(|&band| ATRAC3_SUBBAND_TAB[band + 1] <= spectral_end)
        .count()
}

fn low_budget_abs_threshold(band: usize, band_peak: u8, tbl_index: u8) -> f32 {
    let row = low_budget_spacing_row(band);
    let offset = SONY_LOW_BUDGET_SPACING_BIAS[row * 8 + tbl_index as usize];
    let class = (i32::from(band_peak) - offset)
        .clamp(0, SONY_TONAL_PROMINENCE_THRESHOLDS.len() as i32 - 1) as usize;
    SONY_TONAL_PROMINENCE_THRESHOLDS[class]
}

fn low_budget_spacing_row(band: usize) -> usize {
    let start = ATRAC3_SUBBAND_TAB[band];
    let width = ATRAC3_SUBBAND_TAB[band + 1] - start;
    if width == 32 { 1 } else { start >> 8 }
}

fn scan_low_budget_positions(
    residual: &[f32],
    start: usize,
    end: usize,
    max_candidates: usize,
    threshold: f32,
) -> Option<Vec<usize>> {
    let mut positions = Vec::new();
    let mut position = start;

    while position < end {
        if residual[position].abs() >= threshold {
            if positions.len() == max_candidates {
                return None;
            }
            positions.push(position);
            position += 4;
        } else {
            position += 1;
        }
    }

    Some(positions)
}

fn backtrack_contiguous_peak(
    residual: &[f32],
    previous_band_last_position: Option<usize>,
    mut position: usize,
) -> usize {
    if previous_band_last_position != Some(position.saturating_sub(4)) {
        return position;
    }

    let mut remaining = 4usize;
    while remaining > 1 && position > 0 && residual[position - 1].abs() >= residual[position].abs()
    {
        position -= 1;
        remaining -= 1;
    }
    position
}

fn quantize_low_budget_entry(
    residual: &[f32],
    position: usize,
    band_end: usize,
) -> Result<Option<PendingTonalEntry>> {
    let mut entry = quantize_fixed_clc_entry(residual, position, band_end, LOW_BUDGET_QSTEP)?
        .filter(|candidate| candidate.mantissas.iter().any(|&mantissa| mantissa != 0));
    if let Some(candidate) = entry.as_mut() {
        if candidate.mantissas[0] == 0 && !shift_tonal_entry_right(candidate, band_end) {
            return Ok(None);
        }
    }
    Ok(entry)
}

fn quantize_fixed_clc_entry(
    residual: &[f32],
    position: usize,
    _band_end: usize,
    qstep: u8,
) -> Result<Option<PendingTonalEntry>> {
    let mut samples = [0.0f32; 4];
    for (index, sample) in samples.iter_mut().enumerate() {
        if let Some(&coefficient) = residual.get(position + index) {
            *sample = coefficient;
        }
    }

    let peak_sf = sony_peak_to_sf_index_group(&samples);
    let mut best_sf = peak_sf;
    let mut best_mantissas = [0i8; 4];
    let mut best_error = f32::INFINITY;
    let width = ATRAC3_CLC_LENGTH_TAB[qstep as usize];
    let min_value = -(1i32 << (width - 1));
    let max_value = (1i32 << (width - 1)) - 1;

    let upper = peak_sf.saturating_add(2).min(63);
    for sf_index in (peak_sf..=upper).rev() {
        let scale = scale_factor(sf_index) * ATRAC3_INV_MAX_QUANT[qstep as usize];
        if scale <= 0.0 {
            continue;
        }

        let mut mantissas = [0i8; 4];
        let mut max_error = 0.0f32;
        for (index, &sample) in samples.iter().enumerate() {
            let normalized = sample / scale;
            let quantized = (normalized.round() as i32).clamp(min_value, max_value) as i8;
            mantissas[index] = quantized;
            max_error = max_error.max((normalized - quantized as f32).abs());
        }

        if max_error + 1e-12 < best_error {
            best_error = max_error;
            best_sf = sf_index;
            best_mantissas = mantissas;
        }
    }

    if best_error.is_infinite() {
        return Ok(None);
    }

    strip_even_mantissas(&mut best_mantissas, &mut best_sf);

    Ok(Some(PendingTonalEntry {
        absolute_position: position,
        scale_factor_index: best_sf,
        mantissas: best_mantissas,
    }))
}

fn strip_even_mantissas(mantissas: &mut [i8; 4], scale_factor_index: &mut u8) {
    while *scale_factor_index <= 60
        && mantissas.iter().any(|&mantissa| mantissa != 0)
        && mantissas.iter().all(|&mantissa| mantissa & 1 == 0)
    {
        for mantissa in mantissas.iter_mut() {
            *mantissa /= 2;
        }
        *scale_factor_index += 3;
    }
}

fn shift_tonal_entry_right(entry: &mut PendingTonalEntry, band_end: usize) -> bool {
    let Some(shift) = entry
        .mantissas
        .iter()
        .skip(1)
        .position(|&mantissa| mantissa != 0)
        .map(|index| index + 1)
    else {
        return false;
    };
    let new_position = entry.absolute_position + shift;
    if new_position >= band_end {
        return false;
    }
    if new_position > 0x3fc {
        return true;
    }

    for index in 0..(4 - shift) {
        entry.mantissas[index] = entry.mantissas[index + shift];
    }
    for mantissa in entry.mantissas.iter_mut().skip(4 - shift) {
        *mantissa = 0;
    }
    entry.absolute_position = new_position;
    true
}

fn empty_tonal_result(coded_qmf_bands: u8, low_budget_path: bool) -> TonalExtractionResult {
    TonalExtractionResult {
        tonal_mode_selector: TonalCodingModeSelector::AllVlc,
        tonal_components: Vec::new(),
        tonal_bits: 0,
        coded_qmf_bands,
        tonal_subbands: [false; 32],
        low_budget_path,
    }
}

fn subband_index_for_position(position: usize) -> Option<usize> {
    (0..32).find(|&band| {
        position >= ATRAC3_SUBBAND_TAB[band] && position < ATRAC3_SUBBAND_TAB[band + 1]
    })
}

#[cfg(test)]
mod tests {
    use super::{
        LOW_BUDGET_QSTEP, PendingLowBudgetLayout, PendingTonalEntry, backtrack_contiguous_peak,
        low_budget_abs_threshold, low_budget_layout_cost, low_budget_spacing_row,
        quantize_low_budget_entry, reduce_low_budget_layout, scan_low_budget_positions,
        shift_tonal_entry_right, split_low_budget_layout,
    };
    use crate::atrac3::quant::scale_factor;
    use crate::atrac3::quant_sony::SONY_TONAL_PROMINENCE_THRESHOLDS;
    use crate::atrac3::sound_unit::CodingMode;

    #[test]
    fn maps_low_budget_rows_like_sony_band_groups() {
        assert_eq!(low_budget_spacing_row(0), 0);
        assert_eq!(low_budget_spacing_row(16), 1);
        assert_eq!(low_budget_spacing_row(26), 2);
        assert_eq!(low_budget_spacing_row(30), 3);
    }

    #[test]
    fn uses_dynamic_low_budget_threshold_table() {
        let threshold = low_budget_abs_threshold(16, 16, 3);
        assert_eq!(threshold, SONY_TONAL_PROMINENCE_THRESHOLDS[8]);
    }

    #[test]
    fn low_budget_scan_skips_four_after_hit_and_detects_overflow() {
        let residual = [0.0, 2.0, 0.0, 0.0, 3.0, 0.0, 0.0, 4.0];
        assert_eq!(
            scan_low_budget_positions(&residual, 0, residual.len(), 2, 1.5),
            Some(vec![1, 7])
        );
        assert_eq!(
            scan_low_budget_positions(&residual, 0, residual.len(), 1, 1.5),
            None
        );
    }

    #[test]
    fn backtracks_contiguous_edge_hits_toward_stronger_left_peak() {
        let residual = [0.0, 0.0, 6.0, 5.0, 4.0];
        assert_eq!(backtrack_contiguous_peak(&residual, Some(0), 4), 2);
        assert_eq!(backtrack_contiguous_peak(&residual, Some(1), 4), 4);
    }

    #[test]
    fn strips_leading_zeroes_and_shifts_position() {
        let mut entry = PendingTonalEntry {
            absolute_position: 10,
            scale_factor_index: 8,
            mantissas: [0, 2, -1, 0],
        };
        assert!(shift_tonal_entry_right(&mut entry, 16));
        assert_eq!(entry.absolute_position, 11);
        assert_eq!(entry.mantissas, [2, -1, 0, 0]);
    }

    #[test]
    fn quantized_low_budget_entry_keeps_real_q3_symbols() {
        let mut residual = [0.0f32; 32];
        residual[5] = 0.75;
        residual[6] = -0.5;
        let entry = quantize_low_budget_entry(&residual, 4, 16)
            .unwrap()
            .expect("expected tonal entry");
        assert_eq!(entry.scale_factor_index <= 63, true);
        assert_eq!(
            entry
                .payload(CodingMode::Clc, LOW_BUDGET_QSTEP, 3)
                .unwrap()
                .bit_len(),
            12
        );
        assert!(entry.payload(CodingMode::Vlc, LOW_BUDGET_QSTEP, 3).is_ok());
        let scale = scale_factor(entry.scale_factor_index)
            * crate::atrac3::quant::ATRAC3_INV_MAX_QUANT[LOW_BUDGET_QSTEP as usize];
        assert!(scale > 0.0);
    }

    #[test]
    fn low_budget_split_moves_tail_pair_into_second_component() {
        let layout = PendingLowBudgetLayout {
            components: vec![vec![
                PendingTonalEntry {
                    absolute_position: 4,
                    scale_factor_index: 9,
                    mantissas: [3, 2, 1, -1],
                },
                PendingTonalEntry {
                    absolute_position: 0x104,
                    scale_factor_index: 10,
                    mantissas: [2, 1, 0, 0],
                },
            ]],
            coded_values_minus_one: 3,
        };

        let split = split_low_budget_layout(&layout, 2).expect("split should succeed");
        assert_eq!(split.components.len(), 2);
        assert_eq!(split.components[0][0].mantissas, [3, 2, 0, 0]);
        assert_eq!(split.components[1][0].absolute_position, 6);
        assert_eq!(split.components[1][0].mantissas, [1, -1, 0, 0]);
    }

    #[test]
    fn low_budget_reduce_shifts_entries_and_can_lower_cost() {
        let mut layout = PendingLowBudgetLayout {
            components: vec![vec![
                PendingTonalEntry {
                    absolute_position: 4,
                    scale_factor_index: 9,
                    mantissas: [0, 3, 2, 0],
                },
                PendingTonalEntry {
                    absolute_position: 0x44,
                    scale_factor_index: 11,
                    mantissas: [2, 1, 0, 0],
                },
            ]],
            coded_values_minus_one: 3,
        };
        let before = low_budget_layout_cost(&layout, 1, 10, CodingMode::Clc).expect("cost before");

        reduce_low_budget_layout(&mut layout);

        let after = low_budget_layout_cost(&layout, 1, 10, CodingMode::Clc).expect("cost after");
        assert_eq!(layout.coded_values_minus_one, 1);
        assert_eq!(layout.components[0][0].absolute_position, 5);
        assert_eq!(layout.components[0][0].mantissas, [3, 2, 2, 0]);
        assert!(after < before);
    }
}
