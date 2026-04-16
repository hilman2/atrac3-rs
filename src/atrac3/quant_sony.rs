// Sony reference boundaries dumped from `psp_at3tool.exe`.
// VA 0x0048b520, type `u32[33]` when read as the full subband boundary table.
pub const SONY_SUBBAND_BOUNDARIES: [usize; 33] = [
    0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 288, 320,
    352, 384, 416, 448, 480, 512, 576, 640, 704, 768, 896, 1024,
];

pub const ATRAC3_SUBBAND_TAB: [usize; 33] = SONY_SUBBAND_BOUNDARIES;

// Sony start offsets, derived from the same boundary table.
pub const SONY_SUBBAND_START_OFFSETS: [usize; 32] = [
    0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 288, 320,
    352, 384, 416, 448, 480, 512, 576, 640, 704, 768, 896,
];

// Sony end offsets, derived from the same boundary table.
pub const SONY_SUBBAND_END_OFFSETS: [usize; 32] = [
    8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 288, 320, 352,
    384, 416, 448, 480, 512, 576, 640, 704, 768, 896, 1024,
];

// VA 0x0048cc10, type `i32[32]`.
pub const SONY_ACTIVE_COUNT_WEIGHT: [i32; 32] = [
    8, 8, 8, 8, 8, 8, 8, 8, 16, 16, 16, 16, 16, 16, 16, 16, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
    64, 64, 64, 64, 128, 128,
];

// VA 0x0048cc90, type `i32[32]`.
pub const SONY_ENERGY_THRESHOLD: [i32; 32] = [
    7, 5, 5, 4, 4, 4, 4, 3, 3, 3, 3, 4, 5, 5, 5, 6, 6, 7, 7, 8, 10, 13, 17, 22, 28, 35, 49, 74,
    109, 155, 250, 441,
];

// VA 0x0048cd0c, type `i32[8]`.
pub const SONY_SATURATION_BOUNDARY: [i32; 8] = [441, 1, 2, 2, 2, 4, 6, 6];
// VA 0x0048cd10, type `i32[8]`.
pub const SONY_PROMOTION_BOUNDARY: [i32; 8] = [1, 2, 2, 2, 4, 6, 6, 40];
// VA 0x0048cd28, type `i32[8]`.
pub const SONY_COST_REDUCTION: [i32; 8] = [6, 40, 40, 60, 76, 60, 60, 100];
// VA 0x0048cd44, type `i32[8]`.
pub const SONY_BASE_COST_SCALAR: [i32; 8] = [100, 15, 20, 25, 29, 35, 45, 55];
// VA 0x0048cd60, type `i32[8]`.
pub const SONY_SF_THRESHOLD_OFFSET: [i32; 8] = [55, 3, 5, 7, 9, 12, 15, 18];

// VA 0x004bec08, type `f32[16]`.
pub const SONY_TONAL_PROMINENCE_THRESHOLDS: [f32; 16] = [
    0.03125,
    0.039_372_534,
    0.049_606_282,
    0.0625,
    0.078_745_067,
    0.099_212_565,
    0.125,
    0.157_490_134,
    0.198_425_129,
    0.25,
    0.314_980_268,
    0.396_850_258,
    0.5,
    0.629_960_537,
    0.793_700_516,
    1.0,
];

// VA 0x0048ce70, type `i32[32]`.
pub const SONY_LOW_BUDGET_SPACING_BIAS: [i32; 32] = [
    11, 9, 9, 9, 9, 9, 10, 13, 9, 8, 8, 8, 8, 9, 10, 13, 8, 7, 7, 7, 8, 9, 10, 13, 7, 7, 7, 7, 8,
    9, 9, 9,
];

// VA 0x0048cef0, type `i32[5]`.
pub const SONY_TONAL_SPREAD_Q0: [i32; 5] = [-225, -266, -307, -317, -1024];
// VA 0x004bec50, type `f32`.
pub const SONY_HIGH_BUDGET_ABS_THRESHOLD: f32 = 2.0;
// VA 0x0048bdd8, type `f32[16]`.
pub const SONY_GAIN_LEVELS: [f32; 16] = [
    0.0625, 0.125, 0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 256.0, 512.0, 1024.0,
    2048.0,
];
// VA 0x0048be18, type `f32[8]`.
pub const SONY_GAIN_SHAPING: [f32; 8] = [
    0.0625,
    0.061_422_117,
    0.060_600_22,
    0.060_057_487,
    0.059_819_173,
    0.059_912_838,
    0.060_368_527,
    0.061_219_003,
];
// VA 0x0048e854, type `f32`.
pub const SONY_GAIN_CODE_SCALE: f32 = 14.0;
// VA 0x0048e858, type `f32`.
pub const SONY_GAIN_SILENCE_FLOOR: f32 = 4.791_991_8e12;
// VA 0x0048e85c, type `f32`.
pub const SONY_GAIN_HISTORY_THRESHOLD_A: f32 = 96.0;
// VA 0x0048e860, type `f32`.
pub const SONY_GAIN_HISTORY_THRESHOLD_B: f32 = 2304.0;
// VA 0x0048e864, type `f32`.
pub const SONY_GAIN_HISTORY_THRESHOLD_C: f32 = 19.697_716;
// VA 0x0048e868, type `f32`.
pub const SONY_GAIN_STABLE_FLOOR: f32 = 7.667_187e13;
// VA 0x0048e86c, type `f32`.
pub const SONY_GAIN_PEAK_BUDGET_SCALE: f32 = 9.222_507e-15;
// VA 0x0048e870, type `f32`.
pub const SONY_GAIN_RATIO_CLASS_FACTOR: f32 = std::f32::consts::SQRT_2;
// VA 0x0048e874, type `f32`.
pub const SONY_GAIN_ABS_PROMINENCE_FLOOR: f32 = 4.679_679_2e10;
// VA 0x0048e878, type `f32`.
pub const SONY_GAIN_RELATIVE_PEAK_RATIO: f32 = 1.85;
// VA 0x0048e87c, type `f32`.
pub const SONY_GAIN_GLOBAL_PEAK_FLOOR: f32 = 1.871_871_8e10;

/// DAT_0048b4d0 + 0: magic float used as the "add then truncate" rounding
/// bias in Sony's tonal quantizer (1.5 * 2^23 = 12582912.0).
pub const SONY_TONAL_ROUND_BIAS: f32 = 12582912.0;
/// DAT_0048b4d0 + 4: dithering / overshoot factor of 1.05 used in
/// FUN_00438c20 to test `max_abs_err < scale * 1.05` when picking the
/// best sfIndex for a tonal entry.
pub const SONY_TONAL_OVERSHOOT: f32 = 1.05;

/// DAT_0048b3c0: constant 1.0, used as the dequantization-side multiplier
/// in FUN_00438b60.
pub const SONY_TONAL_DEQUANT_ONE: f32 = 1.0;

/// DAT_0048b5a4: 22×f32 scale LUT. Indexed by
/// `(floor(sfIndex*43/128) + codebook_index) * 3 - sfIndex` where the
/// first term approximates `floor(sfIndex / 3)` via integer math in
/// FUN_00438c20/e30. For codebook=7 the max index reached is 21
/// (value 1008.0), hence the 22-element size.
pub const SONY_TONAL_SCALE_LUT: [f32; 22] = [
    0.0, 30.238106, 38.097626, 48.0, 50.396843, 63.49604, 80.0, 70.55558, 88.894463, 112.0,
    90.714317, 114.292877, 144.0, 151.190521, 190.488129, 240.0, 312.460419, 393.675476, 496.0,
    635.000183, 800.050110, 1008.0,
];

/// DAT_0048b5f4: 8×u32 mask LUT, picks the magnitude bits out of the
/// rounded-short value in FUN_00438b60.
pub const SONY_TONAL_MASK_LUT: [u32; 8] = [0x44480335, 0x447c0000, 0x7, 0x7, 0xf, 0xf, 0x1f, 0x3f];

/// DAT_004bec50: high-budget absolute-magnitude threshold (`2.0`) —
/// FUN_00438830 only accepts tonal candidates whose value is >= this.
pub const SONY_HB_TONAL_ABS_THRESHOLD: f32 = 2.0;

/// Bit width of the sign-extended mantissa value per tbl index, looked up
/// via `*(DAT_004be9cc[tbl*2] + 4)` in the reference encoder. tbl=3 uses
/// the 3-bit separator codebook CB_48BB18, tbl=5 the 4-bit CB_48BB58, and
/// tbl=7 the 6-bit CB_48BBD8. Entries for other tbls have zero sep ptr.
pub const SONY_TONAL_SEP_BIT_LEN: [u8; 8] = [0, 0, 0, 3, 0, 4, 0, 6];

/// DAT_004be9bc magnitude-codebook dispatch by tbl. tbl=0 has no codebook
/// (never reached), tbl=1..7 map to CB_48B618..CB_48B918 as populated by
/// the `s_UnknownVendr` pointer table in the reference EXE.
pub fn sony_tonal_mag_codebook(tbl: u8) -> &'static [(u32, u8)] {
    match tbl {
        1 => &CB_48B618,
        2 => &CB_48B698,
        3 => &CB_48B6D8,
        4 => &CB_48B718,
        5 => &CB_48B798,
        6 => &CB_48B818,
        7 => &CB_48B918,
        _ => &[],
    }
}

/// DAT_004be9cc separator-codebook dispatch by tbl. Only tbl=3, 5, 7 have
/// a non-null separator codebook in Sony's table.
pub fn sony_tonal_sep_codebook(tbl: u8) -> &'static [(u32, u8)] {
    match tbl {
        3 => &CB_48BB18,
        5 => &CB_48BB58,
        7 => &CB_48BBD8,
        _ => &[],
    }
}

/// Quantize one float sample using Sony's "+ magic bias then truncate"
/// trick (`_DAT_0048b4d0 + scale * sample` → SUB42 low 16 bits). This is
/// the IEEE-754 fast-round primitive used throughout the Sony tonal
/// builder. Returns `(signed_mantissa_i16_as_i32, int16_mantissa)` where
/// the first is used for magnitude math and the second for the OR mask.
fn sony_quant_round_i16(scale: f32, sample: f32) -> i32 {
    let biased = SONY_TONAL_ROUND_BIAS + scale * sample;
    let bits = biased.to_bits();
    // SUB42 + assign to `short` == low 16 bits as signed i16.
    let as_i16 = bits as i16;
    as_i16 as i32
}

/// Compute the dequantization scale factor Sony uses, mirroring the
/// `SCALE_LUT[i] - (sfExponent << 23)` IEEE-754 exponent-subtraction
/// trick from FUN_00438e30 / c20 / b60.
fn sony_tonal_step_float(sf_index: u8, tbl_index: u8) -> f32 {
    let u_var5: u32 = (sf_index as u32).wrapping_mul(0x2b0000);
    let approx_sf_div_3 = u_var5 >> 0x17;
    let lut_index = ((approx_sf_div_3 + u32::from(tbl_index)) * 3).wrapping_sub(u32::from(sf_index));
    if (lut_index as usize) >= SONY_TONAL_SCALE_LUT.len() {
        return 0.0;
    }
    let lut_int_bits: u32 = SONY_TONAL_SCALE_LUT[lut_index as usize].to_bits();
    let step_bits: i32 = (lut_int_bits as i32).wrapping_sub((u_var5 & 0x7f800000) as i32);
    f32::from_bits(step_bits as u32)
}

/// Rust-native layout of Sony's param_4 workbench used by FUN_00438830
/// and its sub-routines. Sony's decompile uses raw dword offsets (0x48,
/// 0xce, 0x4a, 0x12c bytes, ...). We keep the semantics 1:1 but give the
/// fields meaningful names.
///
/// Per-channel layout (dwords, relative to channel base):
///   [0x48 or 0xce]: tbl  (5 for channel 0, 7 for channel 1)
///   [0x49 or 0xcf]: count_minus_one (= 3, i.e. 4 mantissas per entry)
///   [0x4a or 0xd0] + cell*8: cell entry count + 7 entry pointers
///   [0x150 + entry*6]: entry storage (mantissas 4 + position 1 + sfIndex 1)
#[derive(Debug, Clone)]
pub struct SonyHbEntry {
    pub mantissas: [u32; 4],
    pub position: u32,
    pub sf_index: u8,
}

/// Low-Budget-Tonal entry layout per PLAN.md Task #6. Used by the
/// FUN_00438ed0 workspace.
#[derive(Debug, Clone, Default)]
pub struct SonyTonalEntry {
    pub mantissas: [u32; 4],
    pub position: u32,
    pub sf_index: i32,
}

#[derive(Debug, Clone, Default)]
pub struct SonyHbCell {
    /// Number of valid entry indices in this cell (0..=7).
    pub count: u8,
    /// Indices into `SonyHbWorkbench::entries`. Only `count` of these are
    /// valid at any time.
    pub entry_indices: [u32; 7],
}

#[derive(Debug, Clone)]
pub struct SonyHbChannel {
    /// Sony's param_4[0x48] / [0xce]. 5 for channel 0, 7 for channel 1.
    pub tbl: u8,
    /// Sony's param_4[0x49] / [0xcf]. Always 3 — signals `count-1`
    /// mantissas, i.e. 4 mantissas per entry.
    pub count_minus_one: u8,
    /// Sony uses position >> 6 as cell index (64-sample cells). For a
    /// 1024-sample spectrum that's 16 cells per channel. Stored at
    /// workbench[channel*0x86 + cell*8 + 0x4a] in the reference encoder.
    pub cells: [SonyHbCell; 16],
}

impl Default for SonyHbChannel {
    fn default() -> Self {
        Self {
            tbl: 0,
            count_minus_one: 3,
            cells: [
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
                SonyHbCell::default(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct SonyHbWorkbench {
    /// Residual spectrum after tonal subtraction (Sony stores this at
    /// param_4+0xc80 bytes = workbench[800] as f32).
    pub residual: Vec<f32>,
    /// Pool of entries. Sony allocates these at param_4[0x150 + i*6]; we
    /// use a Vec and cells hold indices.
    pub entries: Vec<SonyHbEntry>,
    /// Sony's 2 channels — channel 0 uses tbl=5, channel 1 uses tbl=7.
    /// Channel selection in FUN_00438830 is `uVar3 = (peak > 0x1e) as 1`.
    pub channels: [SonyHbChannel; 2],
    /// Number of QMF bands the allocator treats as coded. Sony's
    /// param_4[0x40]. Determines the loop bound on sample position.
    pub local_514: usize,
}

impl SonyHbWorkbench {
    pub fn new(residual: Vec<f32>, local_514: usize) -> Self {
        let mut wb = Self {
            residual,
            entries: Vec::with_capacity(128),
            channels: [SonyHbChannel::default(), SonyHbChannel::default()],
            local_514,
        };
        wb.channels[0].tbl = 5;
        wb.channels[1].tbl = 7;
        wb
    }

    /// Port of Sony FUN_00438d50 (decompile line 43085).
    ///
    /// Scans the cell's 7 slots for the entry with the SMALLEST sfIndex
    /// that is strictly lower than `new_sf_index` (= threshold). If such
    /// a slot exists, evicts that entry, re-adds its dequantized
    /// contribution back to the residual (via FUN_00438e30), shifts the
    /// remaining entries left to fill the gap, decrements the cell count,
    /// and returns `Some(evicted_entry_idx)` for the caller to reuse.
    /// Otherwise returns `None` (the new entry cannot displace any).
    pub fn cell_replace_weakest(
        &mut self,
        channel_idx: usize,
        cell_idx: usize,
        new_sf_index: u8,
    ) -> Option<u32> {
        let tbl = self.channels[channel_idx].tbl;
        let count_minus_one = self.channels[channel_idx].count_minus_one as usize;
        let cell = &self.channels[channel_idx].cells[cell_idx];
        let mut weakest_slot: i32 = -1;
        let mut threshold = new_sf_index as i32;
        for slot in 0..7 {
            let entry_idx = cell.entry_indices[slot] as usize;
            if entry_idx >= self.entries.len() {
                continue;
            }
            let sf = self.entries[entry_idx].sf_index as i32;
            if sf < threshold {
                weakest_slot = slot as i32;
                threshold = sf;
            }
        }
        if weakest_slot < 0 {
            return None;
        }
        let slot = weakest_slot as usize;
        let evicted_idx = self.channels[channel_idx].cells[cell_idx].entry_indices[slot];

        // Call e30: add evicted entry's contribution back to residual.
        let entry = self.entries[evicted_idx as usize].clone();
        sony_tonal_dequant_add(
            &entry.mantissas.map(|m| m as i32),
            entry.position as usize,
            entry.sf_index,
            count_minus_one,
            tbl,
            &mut self.residual,
        );

        // Shift the remaining entries left to fill the gap.
        let cell_mut = &mut self.channels[channel_idx].cells[cell_idx];
        if slot < 6 {
            for i in slot..6 {
                cell_mut.entry_indices[i] = cell_mut.entry_indices[i + 1];
            }
        }
        // Last slot: Sony doesn't write a sentinel, just decrements count.
        cell_mut.count = cell_mut.count.saturating_sub(1);

        Some(evicted_idx)
    }

    /// Port of Sony FUN_00438830 (decompile line 42862).
    ///
    /// Main high-budget tonal builder. Walks through the residual
    /// spectrum and extracts tonal entries (4-sample groups whose peak
    /// magnitude >= 2.0). Each entry is routed to channel 0 (tbl=5) or
    /// channel 1 (tbl=7) based on its peak sfIndex, and placed into a
    /// cell-indexed bucket (cell = position / 64, max 7 per cell). If a
    /// cell is full, the weakest entry is evicted via
    /// `cell_replace_weakest`. Quantization uses
    /// `sony_tonal_quantize_and_subtract` which updates the residual
    /// in-place. Budget tracking terminates the loop when the bit cost
    /// exceeds `(budget - 200)`.
    ///
    /// Returns the total bit cost used by the tonal payload.
    pub fn build_high_budget_tonals(
        &mut self,
        budget_bits: i32,
        au_504: &mut [u32],
        local_400: &mut [u32],
        prev_coded_qmf_bands: &mut u32,
    ) -> i32 {
        // local_1c = running bit cost. Initial 0x16 (22) base.
        let mut local_1c: i32 = 0x16;

        // Update persistent QMF band count, charging 3 bits per new band.
        let cur_coded_qmf_bands = (ATRAC3_SUBBAND_TAB[self.local_514] + 0xff) >> 8;
        if (*prev_coded_qmf_bands as usize) < cur_coded_qmf_bands {
            local_1c += (cur_coded_qmf_bands as i32 - *prev_coded_qmf_bands as i32) * 3;
            *prev_coded_qmf_bands = cur_coded_qmf_bands as u32;
        }

        // Reset all cells and entry pool.
        for ch in &mut self.channels {
            for cell in &mut ch.cells {
                *cell = SonyHbCell::default();
            }
        }
        self.entries.clear();

        // Track per-(channel, qmf_band) active flag. Sony stores these at
        // workbench[channel*0x86 + qmf_band + 0x44]. We mirror with a
        // 2-channel × 4-qmf-band bitmask.
        let mut band_active: [[bool; 4]; 2] = [[false; 4]; 2];

        let coded_samples = ATRAC3_SUBBAND_TAB[self.local_514];

        // Outer loop: iterate until a pass adds no new entries.
        let mut entry_idx_start = 0usize;
        loop {
            let mut local_18 = entry_idx_start;
            let mut position: i32 = 0;

            while position < coded_samples as i32 {
                // abs(sample) >= 2.0 test via bit trick: bits << 1 >= 2.0<<1
                let sample = self.residual[position as usize];
                let sample_bits = sample.to_bits();
                if (sample_bits << 1) < (2.0f32.to_bits() << 1) {
                    position += 1;
                    continue;
                }

                // Clamp entry position to 0x3fc.
                let entry_position = position.min(0x3fc) as u32;

                // Peak sfIndex over the 4-sample group at this position.
                let group_end = ((entry_position as usize) + 4).min(self.residual.len());
                let group = &self.residual[entry_position as usize..group_end];
                let peak_sf = sony_peak_to_sf_index_group(group);

                // Channel selection: 1 if peak > 0x1e, else 0.
                let target_channel = if peak_sf as i32 > 0x1e { 1 } else { 0 };
                let cell_idx = (entry_position >> 6) as usize;
                if cell_idx >= 16 {
                    break;
                }

                // If both channels' cells are full, try to evict.
                let (effective_channel, reuse_idx) = {
                    let cur_full = self.channels[target_channel].cells[cell_idx].count == 7;
                    let other = 1 - target_channel;
                    let other_full = self.channels[other].cells[cell_idx].count == 7;
                    if cur_full && other_full {
                        match self.cell_replace_weakest(target_channel, cell_idx, peak_sf) {
                            Some(evicted) => (target_channel, Some(evicted)),
                            None => {
                                position += 1;
                                continue;
                            }
                        }
                    } else if cur_full {
                        (other, None)
                    } else {
                        (target_channel, None)
                    }
                };

                // Allocate or reuse entry slot.
                let entry_idx = match reuse_idx {
                    Some(idx) => {
                        // Rebuild entry in place.
                        self.entries[idx as usize] = SonyHbEntry {
                            mantissas: [0; 4],
                            position: entry_position,
                            sf_index: peak_sf,
                        };
                        idx
                    }
                    None => {
                        let idx = self.entries.len() as u32;
                        self.entries.push(SonyHbEntry {
                            mantissas: [0; 4],
                            position: entry_position,
                            sf_index: peak_sf,
                        });
                        local_18 += 1;
                        idx
                    }
                };

                // Mark QMF band active, charge 12 bits if newly activated.
                let qmf_band_idx = (entry_position >> 8) as usize;
                if qmf_band_idx < 4 && !band_active[effective_channel][qmf_band_idx] {
                    band_active[effective_channel][qmf_band_idx] = true;
                    local_1c += 0xc;
                }

                // Add entry index to the cell's next slot.
                let cell = &mut self.channels[effective_channel].cells[cell_idx];
                if cell.count < 7 {
                    cell.entry_indices[cell.count as usize] = entry_idx;
                    cell.count += 1;
                }

                // Quantize: FUN_00438b60 selects sfIndex, quantizes, subtracts
                // from residual, returns bit cost.
                let tbl = self.channels[effective_channel].tbl;
                let residual_start = entry_position as usize;
                let residual_end = (residual_start + 4).min(self.residual.len());
                let count = residual_end - residual_start;
                let mut mantissas: [u32; 4] = [0; 4];
                let mut chosen_sf = self.entries[entry_idx as usize].sf_index;
                let _entry_cost = sony_tonal_quantize_and_subtract(
                    &mut self.residual[residual_start..residual_end],
                    &mut mantissas,
                    &mut chosen_sf,
                    count,
                    tbl,
                );
                self.entries[entry_idx as usize].mantissas = mantissas;
                self.entries[entry_idx as usize].sf_index = chosen_sf;

                // Add per-entry overhead bits (0x1c + 0x8 if channel 1).
                local_1c += 0x1c + (effective_channel as i32) * 8;

                // Budget exhaustion check.
                if budget_bits - 200 < local_1c || local_18 > 0x3f {
                    return local_1c;
                }

                // Skip past this group (4 samples).
                position += 4;
            }

            if entry_idx_start == local_18 {
                break;
            }
            entry_idx_start = local_18;
        }

        // Post-processing: recompute band peaks from residual.
        let mut band = 0usize;
        while band < self.local_514 {
            let mut band_peak: u32 = 0;
            let start = ATRAC3_SUBBAND_TAB[band];
            let end = ATRAC3_SUBBAND_TAB[band + 1];
            let mut pos = start;
            while pos < end {
                let g_end = (pos + 4).min(end);
                let group = &self.residual[pos..g_end];
                let peak = sony_peak_to_sf_index_group(group) as u32;
                if (pos >> 2) < local_400.len() {
                    local_400[pos >> 2] = peak;
                }
                if peak > band_peak {
                    band_peak = peak;
                }
                pos += 4;
            }
            let _ = au_504;
            band += 1;
        }

        local_1c
    }
}

/// Port of Sony FUN_00438b60 (decompile line 42981).
///
/// Quantizes `count` samples against the tonal entry's sfIndex using
/// Sony's fast-round primitive, masks with `SONY_TONAL_MASK_LUT[tbl]` to
/// get the stored-magnitude bits, subtracts the dequantized value from
/// the samples (producing the residual), and accumulates the total
/// bitstream cost from the magnitude codebook.
///
/// The `*(param_2 + i*4)` stores in the reference write the MASKED
/// unsigned magnitude (2's-complement truncation) — FUN_00438e30 later
/// sign-extends it using `SONY_TONAL_SEP_BIT_LEN[tbl]`.
///
/// Returns the total bit cost (header 12 + per-sample codebook lengths).
pub fn sony_tonal_quantize_and_subtract(
    samples: &mut [f32],
    entry_mantissas: &mut [u32; 4],
    entry_sf_index: &mut u8,
    count: usize,
    tbl_index: u8,
) -> i32 {
    // First: find best sfIndex (this mirrors FUN_00438b60's call to c20
    // with the same arguments, which writes entry.sfIndex in-place).
    let chosen_sf = sony_tonal_select_sf_index(samples, count, tbl_index);
    *entry_sf_index = chosen_sf;

    let mask = SONY_TONAL_MASK_LUT[tbl_index as usize];
    let scale = sony_tonal_step_float(chosen_sf, tbl_index);
    let scale_recip = SONY_TONAL_DEQUANT_ONE / scale;
    let codebook = sony_tonal_mag_codebook(tbl_index);

    // Base cost is 0xc = 12 bits (entry header).
    let mut total_bits: i32 = 12;
    for i in 0..count {
        let sample = samples[i];
        let mantissa_i16 = sony_quant_round_i16(scale, sample);
        let magnitude_u = (mantissa_i16 as u32) & mask;
        samples[i] = sample - (mantissa_i16 as f32) * scale_recip;
        entry_mantissas[i] = magnitude_u;
        let cb_idx = magnitude_u as usize;
        if cb_idx < codebook.len() {
            total_bits += i32::from(codebook[cb_idx].1);
        }
    }
    total_bits
}

/// Port of Sony FUN_00438c20 (decompile line 43024).
///
/// Finds the best sfIndex for a tonal entry: iterates candidate sfIndex
/// values from `peak_sf + 3` (capped at 0x3f) down to `peak_sf`, for each
/// candidate quantizes all `count` samples using Sony's fast-round
/// primitive, and picks the candidate whose worst-case quantization error
/// divided by its scale is smallest. After picking, performs trailing-
/// zero-stripping (while all mantissas even, shift right and bump
/// sfIndex by 3).
///
/// The decompile has an `extraout_ST0` FPU-leftover ambiguity for the
/// initial `best_ratio` — we use `f32::INFINITY` so the first iteration
/// always commits (the only sensible interpretation of "find best over
/// candidates"). This matches the algorithm's intent regardless of what
/// Sony's caller had on its FPU stack.
pub fn sony_tonal_select_sf_index(samples: &[f32], count: usize, tbl_index: u8) -> u8 {
    let peak_sf = sony_peak_to_sf_index_group(samples) as i32;
    let mut start_sf = peak_sf + 3;
    if start_sf > 0x40 {
        start_sf = 0x40;
    }
    start_sf -= 1;

    if peak_sf > start_sf {
        return 0;
    }

    let mut best_ratio: f32 = f32::INFINITY;
    let mut selected_sf: u8 = 0;
    let mut i_var8 = start_sf;

    while i_var8 >= peak_sf {
        let scale = sony_tonal_step_float(i_var8 as u8, tbl_index);
        let mut or_mask: i32 = 0;
        let mut max_err: f32 = 0.0;

        for &sample in &samples[..count] {
            let scaled = scale * sample;
            let mantissa = sony_quant_round_i16(scale, sample);
            or_mask |= mantissa;
            let err = (scaled - mantissa as f32).abs();
            if err > max_err {
                max_err = err;
            }
        }

        // Sony: `if max_err < scale * best_ratio` — here best_ratio is the
        // running "worst error over scale" threshold.
        if max_err < scale * best_ratio {
            best_ratio = max_err / scale;
            selected_sf = i_var8 as u8;
            // Trailing-zero-strip on the OR-mask while sfIndex < 0x3d.
            let mut u4 = or_mask as u32;
            while (u4 & 1) == 0 && selected_sf < 0x3d {
                u4 >>= 1;
                selected_sf += 3;
            }
        }

        i_var8 -= 1;
    }
    selected_sf
}

/// Port of Sony FUN_00439c20 (decompile line 43830).
///
/// Gain-point picker. Given a 64-sample envelope history (current frame
/// + previous frame), finds up to 7 gain reduction points (position +
/// delta-level) that will be applied to the samples before MDCT.
///
/// Sony's algorithm has two passes (backward from slot 7 down, forward
/// from sample 0 up) that each insert into opposite ends of a shared
/// 7-slot output array, then compact the results.
///
/// Returns `(positions, level_deltas, count)`. `positions[0..count]` are
/// sample positions 0..31. `level_deltas[0..count]` are signed step
/// counts; callers accumulate them from right to left to get absolute
/// level codes (0..=15, 4 = unity).
///
/// Also writes back the `current_peak_out` = dominant peak found, for
/// the caller's persistent state.
pub fn sony_gain_pick_exact(
    history: &[f32],
    previous_peak: f32,
    search_mode: i32,
    current_peak_out: &mut f32,
) -> ([i32; 8], [i32; 8], usize) {
    // 8 slots to match Sony's band-state struct. Backward inserts into
    // slots 5..=6 (max 2 entries), forward into 0..=6. Slot 7 is used as
    // a "scan upper bound" sentinel when backward produced no inserts
    // (i_var10 stays at 7 → positions[7] is read as the scan limit).
    let mut positions = [32i32; 8]; // default sentinel = full 32-sample scan
    let mut deltas = [0i32; 8];

    // Phase 1: coarse 8-slot maxima over history[0..32].
    let mut coarse = [0f32; 8];
    for i in 0..8 {
        let mut m = history[i * 4];
        for j in 1..4 {
            if history[i * 4 + j] > m {
                m = history[i * 4 + j];
            }
        }
        coarse[i] = m;
    }

    // Phase 2: extension scan (Sony sets *(param_1 + 0xc4) = last coarse).
    *current_peak_out = coarse[7];
    let mut fvar1 = coarse[7];
    let scan_end: usize = if search_mode > 0 {
        ((8 - search_mode) * 8) as usize
    } else {
        64
    };
    let mut i_ext = 32usize;
    while i_ext < scan_end && i_ext < history.len() {
        if history[i_ext] > fvar1 {
            fvar1 = history[i_ext];
        }
        i_ext += 1;
    }
    if fvar1 < SONY_GAIN_GLOBAL_PEAK_FLOOR {
        fvar1 = SONY_GAIN_GLOBAL_PEAK_FLOOR;
    }

    // Phase 3: backward pass.
    // Writes into slot `i_var10` which decrements from 7. Break when
    // budget (i_var8) exhausted or i_var10 == 5 (max 2 backward points).
    let mut ratio_threshold = SONY_GAIN_RELATIVE_PEAK_RATIO * fvar1;
    let mut i_var8: i32 = 4; // backward budget
    let mut i_var10: i32 = 7; // output slot index (one past end)
    let mut i_var7: i32 = 8; // coarse slot index (decrementing)

    // Sony accesses `auStack_23 + iVar5` for peak. iVar5 = iVar7*4 - 1
    // decrements by 4 each iter. The underlying data is afStack_20[iVar7-1]
    // when iVar7 >= 1 (i.e. coarse[iVar7-1]).
    while i_var7 >= 1 {
        let slot_idx = (i_var7 - 1) as usize;
        let fvar2 = coarse[slot_idx]; // Sony: peak at this coarse slot
        if fvar1 <= fvar2 {
            if fvar2 > SONY_GAIN_ABS_PROMINENCE_FLOOR && fvar2 > ratio_threshold {
                // Sub-sample position refinement.
                let slot_end_sample = (slot_idx * 4 + 3) as i32; // iVar5

                // default position = iVar7 * 4 = sample at next slot start
                let mut i_var4: i32 = i_var7 * 4;
                if i_var7 != 0 {
                    let pos_plus1 = slot_end_sample + 1;
                    let pos_0 = slot_end_sample;
                    let pos_minus1 = slot_end_sample - 1;
                    // Comma expression semantics: iVar4 = iVar5 always set
                    // before the third check if first two hold.
                    let a = (pos_plus1 as usize) < history.len()
                        && history[pos_plus1 as usize] < ratio_threshold;
                    let b = pos_0 >= 0
                        && (pos_0 as usize) < history.len()
                        && history[pos_0 as usize] < ratio_threshold;
                    if a && b {
                        i_var4 = slot_end_sample;
                        let c = pos_minus1 >= 0
                            && (pos_minus1 as usize) < history.len()
                            && history[pos_minus1 as usize] < ratio_threshold;
                        if c {
                            i_var4 = slot_end_sample - 1;
                        }
                    }
                }

                i_var10 -= 1;
                positions[i_var10 as usize] = i_var4;

                // Step computation via IEEE-754 exponent trick.
                let ratio = (fvar2 / fvar1) * SONY_GAIN_RATIO_CLASS_FACTOR;
                let mut step = ((ratio.to_bits() >> 23) as i32) - 0x7f;
                if i_var8 < step {
                    step = i_var8;
                }
                i_var8 -= step;
                deltas[i_var10 as usize] = -step;

                if i_var8 < 1 || i_var10 == 5 {
                    break;
                }
            }
            ratio_threshold = fvar2 * SONY_GAIN_RELATIVE_PEAK_RATIO;
            fvar1 = fvar2;
        }
        i_var7 -= 1;
    }

    // Phase 4: forward pass.
    let mut fwd_budget: i32 =
        0x83 - ((previous_peak * SONY_GAIN_PEAK_BUDGET_SCALE).to_bits() >> 23) as i32;
    if fwd_budget > 0xf {
        fwd_budget = 0xf;
    }
    let mut i_var7_fwd = fwd_budget - i_var8;
    let mut i_var5: i32 = 0; // forward output index

    if i_var7_fwd > 0 {
        // Forward ratio: 2.0 if search_mode == -1 else 1.6
        let local_2c: f32 = if search_mode == -1 {
            2.0
        } else {
            // 0x3fcccccd = 1.6 in IEEE-754
            f32::from_bits(0x3fcccccd)
        };

        let mut fvar1_fwd = previous_peak;
        if fvar1_fwd < history[0] {
            fvar1_fwd = history[0];
        }
        // i_var10 is now the first filled backward slot.
        let scan_upper: i32 = positions[i_var10 as usize];
        let mut fvar2_fwd = local_2c * fvar1_fwd;

        let mut sample_pos: i32 = 0;
        let mut history_ptr: usize = 0;

        while sample_pos < scan_upper {
            let next_idx = history_ptr + 1;
            if next_idx >= history.len() {
                break;
            }
            let fvar3 = history[next_idx];
            if fvar1_fwd <= fvar3 {
                if fvar3 > SONY_GAIN_ABS_PROMINENCE_FLOOR && fvar3 > fvar2_fwd {
                    positions[i_var5 as usize] = sample_pos;
                    let ratio = (fvar3 / fvar1_fwd) * SONY_GAIN_RATIO_CLASS_FACTOR;
                    let mut step = ((ratio.to_bits() >> 23) as i32) - 0x7f;

                    // Merge with previous if consecutive position and
                    // delta compatible.
                    if i_var5 > 0
                        && positions[(i_var5 - 1) as usize] == sample_pos - 1
                        && deltas[(i_var5 - 1) as usize] <= step
                    {
                        i_var5 -= 1;
                        positions[i_var5 as usize] = sample_pos;
                        i_var7_fwd += deltas[i_var5 as usize];
                        step += deltas[i_var5 as usize];
                    }
                    if i_var7_fwd < step {
                        step = i_var7_fwd;
                    }
                    i_var7_fwd -= step;
                    deltas[i_var5 as usize] = step;
                    i_var5 += 1;
                    if i_var5 == i_var10 || i_var7_fwd < 1 {
                        break;
                    }
                    fvar2_fwd = fvar3 * local_2c;
                    fvar1_fwd = fvar3;
                } else {
                    fvar2_fwd = fvar3 * local_2c;
                    fvar1_fwd = fvar3;
                }
            }
            sample_pos += 1;
            history_ptr += 1;
        }
    }

    // Phase 5: compact — shift backward slots [i_var10..7] into [i_var5..].
    while i_var10 < 7 {
        positions[i_var5 as usize] = positions[i_var10 as usize];
        deltas[i_var5 as usize] = deltas[i_var10 as usize];
        i_var10 += 1;
        i_var5 += 1;
    }

    // Phase 6: accumulate deltas from right to left into absolute levels.
    // Sony initializes the running sum to 4 (= UNITY_GAIN_LEVEL_CODE).
    let count = i_var5 as usize;
    if count > 0 {
        let mut running: i32 = 4;
        for i in (0..count).rev() {
            running += deltas[i];
            deltas[i] = running;
        }
    }

    (positions, deltas, count.min(7))
}

/// Port of the FUN_00439890 "stable-band rescue" branch (decompile
/// lines 43790..=43823).
///
/// When Sony's primary gain picker (FUN_00439c20) returns zero gain
/// points for **Band 0** of a channel whose stored peak has fallen
/// below `SONY_GAIN_STABLE_FLOOR`, the rescue branch inspects the
/// already-updated **Band 1** state of the same frame. If its level log
/// shows at least two distinct entries (`max - min > 1`) AND the NEW
/// Band-0 envelope up to `rescue_position + 1` is also entirely below
/// the stable floor, one synthetic gain point is emitted at position
/// `rescue_position` with level `5`.
///
/// `new_envelope` is Sony's `local_100[...]` (the envelope copied from
/// `param_4 + 0x80`). `stored_peak_old` is Sony's `piVar1[0x30]` value
/// AFTER the pre-pick update at line 43730, i.e. the max of the OLD
/// persistent envelope that was snapshotted before the overwrite.
///
/// Returns `Some((location, level))` on rescue, else `None`.
pub fn sony_gain_rescue_band0(
    new_envelope: &[f32; 32],
    stored_peak_old: f32,
    history_count: u32,
    history_levels: &[i32; 8],
    rescue_position: u32,
    coding_mode_flag: u32,
) -> Option<(u8, u8)> {
    // Sony line 43790: `param_2 == 0 && *(param_1+0x2d64) == 0 &&
    // piVar1[0x32] == 0` — band 0, rate-mode flag clear, per-band flag
    // clear. (Band-0 + flag==0 are checked at the call site; the
    // `*(param_1+0x2d64)` gate maps to `coding_mode_flag` here.)
    if coding_mode_flag != 0 {
        return None;
    }
    // Sony line 43791: `iVar5 = *(param_1+0x2bbc); iVar5 != 0 &&
    // (float)piVar1[0x30] <= _DAT_0048e868`.
    if history_count == 0 {
        return None;
    }
    if stored_peak_old > SONY_GAIN_STABLE_FLOOR {
        return None;
    }
    // Sony lines 43792..=43807: iterate `piVar1 = param_1 + 0x2b14` for
    // `iVar5` entries, tracking min/max. Sony initializes both to 4
    // (UNITY_GAIN_LEVEL_CODE), so an "unpopulated" history still votes
    // for the unity level.
    let mut i_var13: i32 = 4; // min
    let mut i_var14: i32 = 4; // max
    let count = (history_count as usize).min(history_levels.len());
    for &level in &history_levels[..count] {
        if level < i_var13 {
            i_var13 = level;
        }
        if i_var14 < level {
            i_var14 = level;
        }
    }
    // Sony line 43808: `1 < iVar14 - iVar13` — history must show real
    // level variation (>= 2 codes apart).
    if i_var14 - i_var13 <= 1 {
        return None;
    }
    // Sony lines 43809..=43818: every new-envelope slot up to and
    // including `rescue_position` must sit at/below the stable floor.
    let scan_end = (rescue_position as usize + 1).min(new_envelope.len());
    for &slot in &new_envelope[..scan_end] {
        if slot > SONY_GAIN_STABLE_FLOOR {
            return None;
        }
    }
    // Sony lines 43819..=43821: `local_10c = 1; *piVar1 =
    // param_1[0x2af4]; piVar1[8] = 5;` — one gain point, location =
    // rescue_position, level-code = 5.
    let location = rescue_position.min(31) as u8;
    Some((location, 5))
}

/// Port of Sony FUN_00439640 (decompile line 43541).
///
/// Scans the residual spectrum from `start_pos..end_pos` and records up
/// to `max_candidates` positions whose `|sample| * 2 >=
/// SONY_TONAL_PROMINENCE_THRESHOLDS[class_idx] * 2`. When a match is
/// found, the scanner skips ahead by 3 extra samples (so 4 in total),
/// implementing the per-group stride used by the low-budget extractor.
///
/// Returns the number of candidates written. A negative return value
/// signals that `max_candidates` was exhausted (which is how Sony
/// indicates the scan filled up).
pub fn sony_lb_scan_positions(
    class_idx: usize,
    start_pos: usize,
    end_pos: usize,
    max_candidates: usize,
    out_positions: &mut [i32],
    residual: &[f32],
) -> i32 {
    let threshold = SONY_TONAL_PROMINENCE_THRESHOLDS[class_idx];
    let threshold_bits_shifted = (threshold.to_bits() << 1) as u32;
    let mut pos = start_pos;
    let mut count: i32 = 0;
    while pos < end_pos {
        let sample_bits = residual[pos].to_bits();
        // Sony compares `(sample_bits << 1) >= (threshold_bits << 1)` — a
        // sign-strip trick that works because both values are positive
        // floats, and the u32 ordering matches the magnitude ordering.
        if (sample_bits << 1) >= threshold_bits_shifted {
            if (count as usize) == max_candidates {
                return -1;
            }
            out_positions[count as usize] = pos as i32;
            count += 1;
            pos += 3; // +3 now, +1 below → skip 4 samples total
        }
        pos += 1;
    }
    count
}

/// Port of Sony FUN_004395b0 (decompile line 43490).
///
/// Shifts a tonal entry's mantissas left to skip leading zeros, adjusting
/// the entry's position accordingly. Returns the shift amount (0..=3),
/// or `-1` if the shift would push the position past the band end.
///
/// Entry layout (i32 array, offsets in dwords):
///   [0..3]: mantissas
///   [4] (offset 0x10): position
pub fn sony_lb_strip_leading_zeros(
    entry_mantissas: &mut [i32; 4],
    entry_position: &mut u32,
    band_end: u32,
) -> i32 {
    // Find first non-zero mantissa starting at index 1.
    let mut first_nonzero: usize = 1;
    while first_nonzero < 4 {
        if entry_mantissas[first_nonzero] != 0 {
            break;
        }
        first_nonzero += 1;
    }

    let shift = first_nonzero;
    let new_position = *entry_position as i32 + shift as i32;

    if (new_position as u32) >= band_end {
        return -1;
    }
    if new_position > 0x3fc {
        return 0;
    }

    // Shift mantissas left.
    let mut dst = 0;
    let mut src = shift;
    let limit = 4 - shift;
    while dst < limit {
        entry_mantissas[dst] = entry_mantissas[src];
        dst += 1;
        src += 1;
    }

    // Clear freed tail slots based on how many were shifted in.
    match limit {
        1 => {
            entry_mantissas[1] = 0;
            entry_mantissas[2] = 0;
            entry_mantissas[3] = 0;
        }
        2 => {
            entry_mantissas[2] = 0;
            entry_mantissas[3] = 0;
        }
        3 => {
            entry_mantissas[3] = 0;
        }
        _ => {}
    }

    *entry_position = new_position as u32;
    shift as i32
}

/// Port of Sony FUN_00438e30 (decompile line 43133).
///
/// Takes a TonalEntry (`mantissas[0..=count]`, `position`, `sfIndex`) and
/// adds the dequantized mantissas back into the residual spectrum stored
/// at `spectrum[position..position+count+1]`. Used by FUN_00438d50 when a
/// tonal entry is evicted from a full cell — its contribution has to be
/// restored to the residual before the replacement entry is quantized.
///
/// The magic scaling `DAT_0048b3c0 / (SCALE_LUT[...] - sfExponentBits)`
/// reinterprets the LUT entry's raw int bits minus the sfIndex exponent
/// bits as a float — an IEEE-754 exponent subtraction that yields
/// `1.0 / quant_step`.
pub fn sony_tonal_dequant_add(
    entry_mantissas: &[i32; 4],
    entry_position: usize,
    entry_sf_index: u8,
    count_minus_one: usize,
    tbl_index: u8,
    spectrum: &mut [f32],
) {
    // `uVar5 = sfIndex * 0x2b0000`. Used both for `>>23` (= floor(sf*43/128)
    // ≈ floor(sf/3)) and as IEEE-754 exponent bits via `& 0x7f800000`.
    let u_var5: u32 = (entry_sf_index as u32).wrapping_mul(0x2b0000);
    let approx_sf_div_3 = u_var5 >> 0x17;
    let lut_index = (approx_sf_div_3 + u32::from(tbl_index)) * 3 - u32::from(entry_sf_index);
    let lut_int_bits: u32 = SONY_TONAL_SCALE_LUT[lut_index as usize].to_bits();
    let step_bits: i32 = (lut_int_bits as i32).wrapping_sub((u_var5 & 0x7f800000) as i32);
    let step_float = f32::from_bits(step_bits as u32);
    let scale_reciprocal: f32 = SONY_TONAL_DEQUANT_ONE / step_float;

    let sep_len = SONY_TONAL_SEP_BIT_LEN[tbl_index as usize];

    // Iterate from count_minus_one down to 0 inclusive.
    let mut idx = count_minus_one as i32;
    while idx >= 0 {
        let mut mantissa: i32 = entry_mantissas[idx as usize];
        // Sign-extend: if the stored magnitude has the sign bit set,
        // subtract 2^len to get the true negative value.
        if sep_len > 0 && mantissa >= (1i32 << (sep_len - 1)) {
            mantissa = mantissa.wrapping_add(-1i32 << sep_len);
        }
        let sample_pos = entry_position + idx as usize;
        spectrum[sample_pos] += (mantissa as f32) * scale_reciprocal;
        idx -= 1;
    }
}

pub fn sony_active_count_weight(band: usize) -> i32 {
    SONY_ACTIVE_COUNT_WEIGHT[band]
}

pub fn sony_coded_qmf_bands(local_514: usize) -> usize {
    let band_end = ATRAC3_SUBBAND_TAB[local_514.min(32)];
    (band_end + 0xff) >> 8
}

pub fn sony_maybe_grow_coded_band_limit(
    current_limit: usize,
    band: usize,
    active_group_count: usize,
    channel_budget: usize,
    is_follower: bool,
) -> usize {
    // At 132 kbps VLC both channels are coded independently — there is no
    // leader/follower joint-stereo pipeline. The follower gate was a
    // decompile artifact for lower bitrate matrix coding. Keep parameter
    // for compatibility with callers but ignore it.
    let _ = is_follower;
    if band >= 32 {
        return current_limit.clamp(1, 32);
    }

    // Sony reads `*(byte *)(&DAT_0048b520 + iVar3) & 0x80`. DAT_0048b520 is the
    // subband-boundary u32 table read byte-addressed — the 32 bytes at that
    // address are the low bytes of u32[0..7] (values 0, 8, 16, 24, 32, 40, 48,
    // 56) so none has bit 0x80 set. The gate `(flag & 0x80) == 0` is therefore
    // always true in the reference encoder; only the budget checks matter.
    let within_primary_gate = active_group_count * 8 < channel_budget;
    let within_secondary_gate = active_group_count * 16 < channel_budget;
    if !within_primary_gate && !within_secondary_gate {
        return current_limit.clamp(1, 32);
    }

    let candidate = if band > 0x1a {
        (band + 2).min(0x20)
    } else {
        0x1c
    };
    current_limit.max(candidate).clamp(1, 32)
}

pub fn sony_gain_reserve_bits(local_514: usize, gain_point_counts: &[u8; 4]) -> i32 {
    let coded_qmf_bands = sony_coded_qmf_bands(local_514).min(gain_point_counts.len());
    let gain_point_sum: i32 = gain_point_counts[..coded_qmf_bands]
        .iter()
        .map(|&count| i32::from(count))
        .sum();
    (gain_point_sum * 3 + coded_qmf_bands as i32) * 3
}

pub fn sony_hf_trim_refund_bits(
    local_514: usize,
    gain_point_counts: &[u8; 4],
    tonal_component_count: u8,
    tonal_active_qmf_bands: &[bool; 4],
) -> i32 {
    let initial_coded_qmf_bands = sony_coded_qmf_bands(local_514)
        .min(gain_point_counts.len())
        .min(tonal_active_qmf_bands.len());
    let mut coded_qmf_bands = initial_coded_qmf_bands;

    while coded_qmf_bands > 1 {
        let qmf_band = coded_qmf_bands - 1;
        if gain_point_counts[qmf_band] != 0 || tonal_active_qmf_bands[qmf_band] {
            break;
        }
        coded_qmf_bands -= 1;
    }

    (i32::from(tonal_component_count) + 3)
        * (initial_coded_qmf_bands as i32 - coded_qmf_bands as i32)
}

pub fn sony_score_to_tbl(score: i32, bvar1: u8, min: i32) -> i32 {
    let shifted = score >> (bvar1 & 0x1f);
    shifted.clamp(min, 7)
}

pub fn sony_peak_to_sf_index_group(coefficients: &[f32]) -> u8 {
    let mut max_abs_bits = 0u32;
    for &coefficient in coefficients {
        let doubled = coefficient.to_bits() << 1;
        if doubled > max_abs_bits {
            max_abs_bits = doubled;
        }
    }
    if max_abs_bits == 0 {
        return 0;
    }

    let exponent = (max_abs_bits >> 24) as i32;
    let mantissa = max_abs_bits & 0x00ff_ffff;

    let mut sf_index = 3 * exponent - 0x16c;
    if mantissa > 0x0096_5fe9 {
        sf_index += 1;
    }
    if mantissa < 0x0042_8a30 {
        sf_index -= 1;
    }

    if !(0..=0x3f).contains(&sf_index) {
        0
    } else {
        sf_index as u8
    }
}

pub fn sony_group_sf_indices(coefficients: &[f32]) -> Vec<u8> {
    coefficients
        .chunks(4)
        .map(sony_peak_to_sf_index_group)
        .collect()
}

// FUN_00438630 @ 00438630 — Ghidra 42707-42735.
// Band-weise Bit-Kostenschätzer: Basis-Kosten proportional zu
// Sample-Zahl und tbl-Gewicht, minus eines Rabatts pro Gruppe deren
// sfIndex unter der tbl-abhängigen Schwelle liegt.
//
// Sony-Formel (42723): cost = SONY_BASE_COST_SCALAR[tbl] *
//   SONY_ACTIVE_COUNT_WEIGHT[band] + 0x3c
// Sony-Schleife (42724-42733): iteriert alle Groups im Band
// (piVar4..piVar1, Schrittweite 2 mit unroll). In Rust: der Caller
// übergibt bereits den per-Band-Slice aus `state.group_sf[band]`
// (Länge = active_count/4), daher reicht ein einfacher for-Loop.
//
// Sony hat KEINEN tbl==0 Short-Circuit; alle Callsites guarden ihn
// aber vorher. Der defensive Guard hier bleibt drin, weil ein
// Entfernen in Iteration #3 eine SNR-Regression von 14.47→8.77 auf
// HateMe ausgelöst hat (vermutlich über einen bisher unentdeckten
// indirekten Pfad). Wenn der echte Callgraph 1:1 portiert ist, fällt
// der Guard weg.
pub fn sony_band_bit_cost(
    group_sf_indices: &[u8],
    band: usize,
    tbl_index: u8,
    sf_index: i32,
) -> i32 {
    if tbl_index == 0 {
        return 0;
    }
    let table_index = tbl_index as usize;
    let threshold = sf_index - SONY_SF_THRESHOLD_OFFSET[table_index];
    let mut cost = SONY_BASE_COST_SCALAR[table_index] * SONY_ACTIVE_COUNT_WEIGHT[band] + 0x3c;
    for &group_sf_index in group_sf_indices {
        if i32::from(group_sf_index) < threshold {
            cost -= SONY_COST_REDUCTION[table_index];
        }
    }
    cost
}

// =====================================================================
// FUN_004353f0 (gain-application + 256-pt MDCT + overlap-add) constants.
// Dumped via `dump_tonal_consts.py` from `psp_at3tool.exe`.
// =====================================================================

include!("quant_sony_gain_apply.rs");

#[cfg(test)]
mod tests {
    use super::{
        SONY_ENERGY_THRESHOLD, SONY_GAIN_LEVELS, SONY_GAIN_SHAPING, SONY_LOW_BUDGET_SPACING_BIAS,
        SONY_PROMOTION_BOUNDARY, SONY_SATURATION_BOUNDARY, sony_band_bit_cost,
        sony_gain_reserve_bits, sony_hf_trim_refund_bits, sony_maybe_grow_coded_band_limit,
        sony_peak_to_sf_index_group, sony_score_to_tbl,
    };

    #[test]
    fn exposes_separate_saturation_and_promotion_tables() {
        assert_eq!(SONY_SATURATION_BOUNDARY, [441, 1, 2, 2, 2, 4, 6, 6]);
        assert_eq!(SONY_PROMOTION_BOUNDARY, [1, 2, 2, 2, 4, 6, 6, 40]);
        assert_eq!(SONY_ENERGY_THRESHOLD[31], 441);
    }

    #[test]
    fn exposes_full_gain_level_table() {
        assert_eq!(SONY_GAIN_LEVELS.len(), 16);
        assert_eq!(SONY_GAIN_LEVELS[4], 1.0);
        assert_eq!(SONY_GAIN_LEVELS[8], 16.0);
        assert_eq!(SONY_GAIN_LEVELS[15], 2048.0);
    }

    #[test]
    fn exposes_gain_shaping_table() {
        assert_eq!(SONY_GAIN_SHAPING.len(), 8);
        assert!((SONY_GAIN_SHAPING[0] - 0.0625).abs() < 1e-7);
        assert!((SONY_GAIN_SHAPING[4] - 0.059_819_173).abs() < 1e-7);
        assert!((SONY_GAIN_SHAPING[7] - 0.061_219_003).abs() < 1e-7);
    }

    #[test]
    fn ports_sony_score_to_tbl_clamp() {
        assert_eq!(sony_score_to_tbl(0x800, 10, 1), 2);
        assert_eq!(sony_score_to_tbl(-0x400, 10, 1), 1);
        assert_eq!(sony_score_to_tbl(0x4000, 10, 1), 7);
    }

    #[test]
    fn ports_group_peak_to_sf_index() {
        assert_eq!(sony_peak_to_sf_index_group(&[1.0]), 16);
        assert_eq!(sony_peak_to_sf_index_group(&[0.03125]), 1);
        assert_eq!(sony_peak_to_sf_index_group(&[0.0, -2.0, 0.0, 0.0]), 19);
    }

    #[test]
    fn ports_band_cost_formula() {
        assert_eq!(sony_band_bit_cost(&[0, 0], 0, 1, 10), 100);
    }

    #[test]
    fn ports_gain_reserve_formula() {
        assert_eq!(sony_gain_reserve_bits(28, &[1, 2, 3, 4]), 63);
        assert_eq!(sony_gain_reserve_bits(32, &[1, 2, 3, 4]), 102);
    }

    #[test]
    fn ports_phase1_band_limit_growth_gate() {
        assert_eq!(sony_maybe_grow_coded_band_limit(24, 0, 8, 128, false), 28);
        assert_eq!(sony_maybe_grow_coded_band_limit(28, 27, 31, 256, false), 29);
        assert_eq!(sony_maybe_grow_coded_band_limit(24, 8, 32, 128, false), 24);
        assert_eq!(sony_maybe_grow_coded_band_limit(24, 27, 32, 256, true), 24);
    }

    #[test]
    fn ports_hf_trim_refund_formula() {
        assert_eq!(
            sony_hf_trim_refund_bits(32, &[1, 0, 0, 0], 1, &[true, false, false, false]),
            12
        );
        assert_eq!(
            sony_hf_trim_refund_bits(32, &[1, 2, 0, 0], 2, &[true, true, false, false]),
            10
        );
    }

    #[test]
    fn exposes_full_low_budget_spacing_table() {
        assert_eq!(SONY_LOW_BUDGET_SPACING_BIAS.len(), 32);
        assert_eq!(
            SONY_LOW_BUDGET_SPACING_BIAS,
            [
                11, 9, 9, 9, 9, 9, 10, 13, 9, 8, 8, 8, 8, 9, 10, 13, 8, 7, 7, 7, 8, 9, 10, 13, 7,
                7, 7, 7, 8, 9, 9, 9,
            ]
        );
    }
}
/// Sony tonal VLC codebook @ VA 0x0048b618; 16 (code, bit_len) entries.
pub const CB_48B618: [(u32, u8); 16] = [
    (0x00000000, 1),
    (0x80000000, 3),
    (0x00000000, 1),
    (0xa0000000, 3),
    (0xc0000000, 4),
    (0xe0000000, 5),
    (0x00000000, 1),
    (0xe8000000, 5),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0xd0000000, 4),
    (0xf0000000, 5),
    (0x00000000, 1),
    (0xf8000000, 5),
];

/// Sony tonal VLC codebook @ VA 0x0048b698; 8 (code, bit_len) entries.
pub const CB_48B698: [(u32, u8); 8] = [
    (0x00000000, 1),
    (0x00800000, 3),
    (0x00c00000, 3),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00e00000, 3),
    (0x00a00000, 3),
];

/// Sony tonal VLC codebook @ VA 0x0048b6d8; 8 (code, bit_len) entries.
pub const CB_48B6D8: [(u32, u8); 8] = [
    (0x00000000, 1),
    (0x00800000, 3),
    (0x00c00000, 4),
    (0x00e00000, 4),
    (0x00000000, 1),
    (0x00f00000, 4),
    (0x00d00000, 4),
    (0x00a00000, 3),
];

/// Sony tonal VLC codebook @ VA 0x0048b718; 16 (code, bit_len) entries.
pub const CB_48B718: [(u32, u8); 16] = [
    (0x00000000, 1),
    (0x00800000, 3),
    (0x00c00000, 4),
    (0x00e00000, 5),
    (0x00f00000, 5),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00000000, 1),
    (0x00f80000, 5),
    (0x00e80000, 5),
    (0x00d00000, 4),
    (0x00a00000, 3),
];

/// Sony tonal VLC codebook @ VA 0x0048b798; 16 (code, bit_len) entries.
pub const CB_48B798: [(u32, u8); 16] = [
    (0x00000000, 2),
    (0x00400000, 3),
    (0x00800000, 4),
    (0x00a00000, 4),
    (0x00e00000, 5),
    (0x00f00000, 6),
    (0x00f80000, 6),
    (0x00c00000, 4),
    (0x00000000, 2),
    (0x00d00000, 4),
    (0x00fc0000, 6),
    (0x00f40000, 6),
    (0x00e80000, 5),
    (0x00b00000, 4),
    (0x00900000, 4),
    (0x00600000, 3),
];

/// Sony tonal VLC codebook @ VA 0x0048b818; 32 (code, bit_len) entries.
pub const CB_48B818: [(u32, u8); 32] = [
    (0x00000000, 3),
    (0x00200000, 4),
    (0x00400000, 4),
    (0x00600000, 4),
    (0x00a00000, 5),
    (0x00b00000, 5),
    (0x00c00000, 5),
    (0x00d00000, 6),
    (0x00d80000, 6),
    (0x00e00000, 6),
    (0x00e80000, 6),
    (0x00f00000, 7),
    (0x00f40000, 7),
    (0x00f80000, 7),
    (0x00fc0000, 7),
    (0x00800000, 4),
    (0x00000000, 3),
    (0x00900000, 4),
    (0x00fe0000, 7),
    (0x00fa0000, 7),
    (0x00f60000, 7),
    (0x00f20000, 7),
    (0x00ec0000, 6),
    (0x00e40000, 6),
    (0x00dc0000, 6),
    (0x00d40000, 6),
    (0x00c80000, 5),
    (0x00b80000, 5),
    (0x00a80000, 5),
    (0x00700000, 4),
    (0x00500000, 4),
    (0x00300000, 4),
];

/// Sony tonal VLC codebook @ VA 0x0048b918; 64 (code, bit_len) entries.
pub const CB_48B918: [(u32, u8); 64] = [
    (0x00000000, 3),
    (0x00400000, 5),
    (0x00500000, 5),
    (0x00600000, 5),
    (0x00700000, 5),
    (0x00800000, 5),
    (0x00900000, 6),
    (0x00980000, 6),
    (0x00a00000, 6),
    (0x00a80000, 6),
    (0x00b00000, 6),
    (0x00b80000, 6),
    (0x00c00000, 6),
    (0x00c80000, 6),
    (0x00d00000, 7),
    (0x00d40000, 7),
    (0x00d80000, 7),
    (0x00dc0000, 7),
    (0x00e00000, 7),
    (0x00e40000, 7),
    (0x00e80000, 7),
    (0x00ec0000, 8),
    (0x00ee0000, 8),
    (0x00f00000, 8),
    (0x00f20000, 8),
    (0x00f40000, 8),
    (0x00f60000, 8),
    (0x00f80000, 8),
    (0x00fa0000, 8),
    (0x00fc0000, 8),
    (0x00fe0000, 8),
    (0x00200000, 4),
    (0x00000000, 3),
    (0x00300000, 4),
    (0x00ff0000, 8),
    (0x00fd0000, 8),
    (0x00fb0000, 8),
    (0x00f90000, 8),
    (0x00f70000, 8),
    (0x00f50000, 8),
    (0x00f30000, 8),
    (0x00f10000, 8),
    (0x00ef0000, 8),
    (0x00ed0000, 8),
    (0x00ea0000, 7),
    (0x00e60000, 7),
    (0x00e20000, 7),
    (0x00de0000, 7),
    (0x00da0000, 7),
    (0x00d60000, 7),
    (0x00d20000, 7),
    (0x00cc0000, 6),
    (0x00c40000, 6),
    (0x00bc0000, 6),
    (0x00b40000, 6),
    (0x00ac0000, 6),
    (0x00a40000, 6),
    (0x009c0000, 6),
    (0x00940000, 6),
    (0x00880000, 5),
    (0x00780000, 5),
    (0x00680000, 5),
    (0x00580000, 5),
    (0x00480000, 5),
];

/// Sony tonal VLC codebook @ VA 0x0048bb18; 8 (code, bit_len) entries.
pub const CB_48BB18: [(u32, u8); 8] = [
    (0x00000000, 3),
    (0x00200000, 3),
    (0x00400000, 3),
    (0x00600000, 3),
    (0x00000000, 3),
    (0x00a00000, 3),
    (0x00c00000, 3),
    (0x00e00000, 3),
];

/// Sony tonal VLC codebook @ VA 0x0048bb58; 16 (code, bit_len) entries.
pub const CB_48BB58: [(u32, u8); 16] = [
    (0x00000000, 4),
    (0x00100000, 4),
    (0x00200000, 4),
    (0x00300000, 4),
    (0x00400000, 4),
    (0x00500000, 4),
    (0x00600000, 4),
    (0x00700000, 4),
    (0x00000000, 4),
    (0x00900000, 4),
    (0x00a00000, 4),
    (0x00b00000, 4),
    (0x00c00000, 4),
    (0x00d00000, 4),
    (0x00e00000, 4),
    (0x00f00000, 4),
];

/// Sony tonal VLC codebook @ VA 0x0048bbd8; 32 (code, bit_len) entries.
pub const CB_48BBD8: [(u32, u8); 32] = [
    (0x00000000, 6),
    (0x00040000, 6),
    (0x00080000, 6),
    (0x000c0000, 6),
    (0x00100000, 6),
    (0x00140000, 6),
    (0x00180000, 6),
    (0x001c0000, 6),
    (0x00200000, 6),
    (0x00240000, 6),
    (0x00280000, 6),
    (0x002c0000, 6),
    (0x00300000, 6),
    (0x00340000, 6),
    (0x00380000, 6),
    (0x003c0000, 6),
    (0x00400000, 6),
    (0x00440000, 6),
    (0x00480000, 6),
    (0x004c0000, 6),
    (0x00500000, 6),
    (0x00540000, 6),
    (0x00580000, 6),
    (0x005c0000, 6),
    (0x00600000, 6),
    (0x00640000, 6),
    (0x00680000, 6),
    (0x006c0000, 6),
    (0x00700000, 6),
    (0x00740000, 6),
    (0x00780000, 6),
    (0x007c0000, 6),
];

/// Slot-based accessor (PLAN Task #10). Sony's FUN_00438ed0 indexes
/// codebooks by tbl 0..7 via DAT_004be9bc. Slot 0 is unused; slots 1..7
/// map to the primary magnitude codebooks already ported above.
pub const SONY_CODEBOOK_TABLES: [&[(u32, u8)]; 8] = [
    &[],
    &CB_48B618,
    &CB_48B698,
    &CB_48B6D8,
    &CB_48B718,
    &CB_48B798,
    &CB_48B818,
    &CB_48B918,
];

/// Slot-based accessor for separator codebooks (DAT_004be9cc).
/// Only slots 3, 5, 7 have non-empty separator codebooks.
pub const SONY_CODEBOOK_SEP_TABLES: [&[(u32, u8)]; 8] =
    [&[], &[], &[], &CB_48BB18, &[], &CB_48BB58, &[], &CB_48BBD8];

/// VA 0x0048cdf0, i32[16]. Bit-Kosten-LUT für tbl=1 (movmskps-Index
/// über 4-Sample-Gruppe, bit i = (|sample_i| != 0)).
pub const SONY_TBL1_GROUP_BITCOST_LUT: [i32; 16] = [
    2, 5, 4, 6, 5, 8, 7, 9, 4, 7, 6, 8, 6, 9, 8, 10,
];

/// Port von Sony FUN_004367b0 (Ghidra 41442-41586).
///
/// Echter residual-basierter Band-Bitkosten-Estimator. Sony hinterlegt
/// diesen Function-Pointer bei `param_2[0x19df]` (Offset 0x677c) und
/// ruft ihn in Phase 9 (FUN_00437bb0, Zeilen 42599, 42675, 42687) auf.
/// Rückgabe: 6 Basis-Bits + Σ Per-Sample-Codebook-Bit-Längen.
///
/// Signatur Sony: `(param_1=tbl, param_2=sf_index, param_3=count,
/// param_4=residual_spectrum_ptr)`. Scale via IEEE-Trick aus
/// SONY_TONAL_SCALE_LUT und sf_index-Exponent, identisch zu
/// `sony_tonal_step_float`.
///
/// Für tbl=1 nutzt Sony `DAT_0048cdf0` (LUT über movmskps-Ergebnis der
/// 4-Sample-Gruppe). Für tbl>=2 addiert Sony pro Sample
/// `codebook[mag_bits & mask][bit_length]`.
///
/// Die SSE-Entrollung 16-Sample-Chunks (Ghidra 41487-41515,
/// 41518-41554) ist mathematisch äquivalent zu einem Per-Sample-Loop,
/// so dass der Port ohne movmskps/SIMD auskommt.
pub fn sony_band_residual_bit_cost(
    tbl: u8,
    sf_index: u8,
    count: usize,
    spectrum: &[f32],
) -> i32 {
    if count == 0 || count > spectrum.len() {
        return 6;
    }
    let scale = sony_tonal_step_float(sf_index, tbl);
    let bias_bits = SONY_TONAL_ROUND_BIAS.to_bits();
    let mut total: i32 = 6;

    if tbl == 1 {
        // tbl=1: 4-Sample-Gruppe → movmskps → DAT_0048cdf0.
        let mut i = 0usize;
        while i + 4 <= count {
            let mut mask: usize = 0;
            for j in 0..4 {
                let rounded = spectrum[i + j] * scale + SONY_TONAL_ROUND_BIAS;
                if rounded.to_bits() != bias_bits {
                    mask |= 1 << j;
                }
            }
            total += SONY_TBL1_GROUP_BITCOST_LUT[mask & 0xf];
            i += 4;
        }
        return total;
    }

    let codebook_mask: u32 = match tbl {
        2 | 3 => 0x7,
        4 | 5 => 0xf,
        6 => 0x1f,
        7 => 0x3f,
        _ => return total, // tbl=0 nicht aufgerufen in Phase 9 (Guard im Caller)
    };
    let codebook = sony_tonal_mag_codebook(tbl);
    if codebook.is_empty() {
        return total;
    }
    for &sample in &spectrum[..count] {
        let rounded = sample * scale + SONY_TONAL_ROUND_BIAS;
        let idx = (rounded.to_bits() & codebook_mask) as usize;
        if idx < codebook.len() {
            total += i32::from(codebook[idx].1);
        }
    }
    total
}

// ================================================================
// Low-Budget Tonal Builder — FUN_00438ed0 and helpers.
//
// Ghidra: `ghidra_output/decompiled.c` lines 43167 (FUN_00438ed0),
// 43367 (FUN_004393d0), 43405 (FUN_00439470), 43490 (FUN_004395b0).
//
// This block is a 1:1 Rust port. Byte offsets from Sony's workspace
// are preserved as comments on each field. No heuristics, no tuning.
// ================================================================

/// DAT_004be9bc `+0xc` slot, `+4` field: per-tbl SEP-codebook bit width.
/// Matches SONY_TONAL_SEP_BIT_LEN. Used in FUN_00438ed0 repack-cost math
/// (Ghidra 43325: `*(DAT_004be9bc + tbl*8 + 0xc) + 4`).
pub const SONY_LB_SEP_UNIT_BITS: [i32; 8] = [0, 0, 0, 3, 0, 4, 0, 6];

/// DAT_004be9cc `+0` slot, `+4` field: per-tbl MAG-codebook log2(size).
/// For the codebooks Sony ships these are power-of-two sized so this
/// doubles as the "fixed-width-repack" cost. Used in FUN_00438ed0 cost
/// math (Ghidra 43327: `*(&DAT_004be9cc + tbl*8) + 4`).
pub const SONY_LB_MAG_UNIT_BITS: [i32; 8] = [0, 0, 0, 3, 0, 4, 0, 6];

/// A single entry in the low-budget tonal workspace. Matches the 6-dword
/// / 24-byte layout at `workspace + 0x540 + i*0x18`.
///
/// Dword layout (Sony):
///   [0..3] = mantissas (u32, stored post-quant with low `sep_bits` bits)
///   [4]    = position (i32 sample index, 0..=0x3fc)
///   [5]    = sfIndex  (i32, 0..=0x3f)
#[derive(Debug, Clone, Copy, Default)]
pub struct LbEntry {
    pub mantissas: [u32; 4],
    pub position: i32,
    pub sf_index: i32,
}

/// One cell of the LB workspace — 8 dwords / 32 bytes, matching Sony's
/// `workspace + 0x128 + cell_idx*0x20` layout.
///
/// Dword layout (Sony):
///   [0]    = count (number of valid entry refs in this cell)
///   [1..8] = entry_refs (pointers in Sony, indices in our port)
#[derive(Debug, Clone, Copy, Default)]
pub struct LbCell {
    pub count: i32,
    pub entry_refs: [u32; 7],
}

/// Sony-exact Phase-1 + Phase-4 inputs for the low-budget tonal builder
/// (FUN_00438ed0). 1:1 port of the caller-side arrays produced by
/// `FUN_00437bb0` between Ghidra lines 42322 (`local_514 = param_1[0xb57]`)
/// and 42400 (call to `FUN_00438ed0`).
///
/// Fields mirror Sony's `param_2` workspace at the point FUN_00438ed0 is
/// invoked:
///   `band_count`     = `param_2[0x40]`             (local_514)
///   `band_peak[32]`  = `param_2[band + 0x20]`      (Phase-1 peak sfIndex,
///                                                  positive; only bands
///                                                  where Phase-1 set a
///                                                  non-zero peak)
///   `band_state_low` = `param_2[band]` for bands 0..local_514 (Phase-4
///                                                  tbl = FUN_004387b0)
///   `band_score[32]` = `auStack_504[band+1]` after Phase-4 overwrite
///                                                  (= peak*256 - total_sum)
///   `base_bits`      = `*(param_2 + 0x104)` set by FUN_004386a0
///                                                  (= (ATRAC3_SUBBAND_TAB
///                                                  [local_514]+0xff)>>8)
///   `bvar1`          = Sony Phase-2 mode flag (10 or 11)
#[derive(Debug, Clone)]
pub struct SonyLbPhase1Inputs {
    pub band_count: i32,
    pub band_peak: [i32; 32],
    pub band_state_low: [i32; 32],
    pub band_score: [i32; 32],
    pub base_bits: i32,
    pub bvar1: u8,
}

/// 1:1 port of Sony FUN_00437bb0 prelude (Ghidra 42322-42400) — produces
/// the caller-side `param_2` / `auStack_504` arrays FUN_00438ed0 consumes.
///
/// Inputs:
/// * `coefficients`        — 1024-sample spectrum (Sony's `param_1` copied
///                            into `param_2 + 800` and grouped by
///                            `param_2[0x19da]` into `local_400`).
/// * `channel_budget_bits` — Sony's `param_1[0xb58]`.
/// * `is_follower`         — Sony's `param_1[0xb59] != 0`.
/// * `initial_band_limit`  — Sony's `param_1[0xb57]` (persistent
///                            per-channel state).
pub fn sony_phase1_lb_inputs(
    coefficients: &[f32],
    channel_budget_bits: i32,
    is_follower: bool,
    initial_band_limit: u8,
) -> SonyLbPhase1Inputs {
    let mut local_514: u32 = (initial_band_limit as u32).clamp(1, 32);

    // Ghidra 42327: group 1024 samples into 256 sfIndex groups.
    let mut local_400 = [0u8; 256];
    for (i, chunk) in coefficients.chunks(4).enumerate().take(256) {
        local_400[i] = sony_peak_to_sf_index_group(chunk);
    }

    let mut stack_520: u32 = 0; // uStack_520 — running total sum
    let mut stack_518: u32 = 0; // uStack_518 — running global peak
    let mut stack_510: u32 = 0; // iStack_510 — cumulative hot-group count
    let mut band_peak = [0i32; 32];
    let mut au_504 = [0i32; 33]; // auStack_504[0..=32]; au_504[band+1] set per band

    // Ghidra 42333-42371: per-band loop over groups.
    let mut group_idx = 0usize;
    for band in 0..32 {
        let band_group_end = ATRAC3_SUBBAND_TAB[band + 1] >> 2;
        let mut sum_val: u32 = 0;
        let mut peak_val: u32 = 0;
        while group_idx < band_group_end {
            let v = local_400[group_idx] as u32;
            sum_val = sum_val.wrapping_add(v);
            if peak_val < v {
                peak_val = v;
            }
            if v > 7 {
                stack_510 = stack_510.wrapping_add(1);
            }
            group_idx += 1;
        }
        au_504[band + 1] = stack_510 as i32;

        if stack_518 < peak_val {
            stack_518 = peak_val;
            local_514 = sony_maybe_grow_coded_band_limit(
                local_514 as usize,
                band,
                stack_510 as usize,
                channel_budget_bits as usize,
                is_follower,
            ) as u32;
        } else if (sum_val as i32) < SONY_ENERGY_THRESHOLD[band] || (peak_val as i32) < 3 {
            sum_val = 0;
            peak_val = 0;
        }
        band_peak[band] = peak_val as i32;
        stack_520 = stack_520.wrapping_add(sum_val);
    }
    let local_514 = local_514.clamp(1, 32) as i32;

    // Ghidra 42373-42376: bVar1 mode flag.
    let bvar1: u8 = if (channel_budget_bits as i64 * 2)
        < (au_504[local_514 as usize] as i64 * 0xc)
    {
        11
    } else {
        10
    };

    // Ghidra 42750-42751 (FUN_004386a0): base_bits stored at +0x104.
    let base_bits: i32 =
        ((ATRAC3_SUBBAND_TAB[local_514 as usize] + 0xff) >> 8) as i32;

    // Ghidra 42383-42397 (else branch): overwrite loop for bands
    // 0..local_514.
    //   param_2[band]     = (peak > 2) ? FUN_004387b0(peak*256 - total_sum,
    //                                                 bvar1, 1) : 0
    //   auStack_504[band+1] = peak*256 - total_sum (score)
    let mut band_state_low = [0i32; 32];
    let mut band_score = [0i32; 32];
    for band in 0..(local_514 as usize) {
        let peak = band_peak[band];
        let score = peak * 256 - stack_520 as i32;
        band_score[band] = score;
        if peak > 2 {
            band_state_low[band] = sony_score_to_tbl(score, bvar1, 1);
        } else {
            band_state_low[band] = 0;
        }
    }

    SonyLbPhase1Inputs {
        band_count: local_514,
        band_peak,
        band_state_low,
        band_score,
        base_bits,
        bvar1,
    }
}

/// Low-budget tonal workspace (param_3 in FUN_00438ed0).
///
/// Preserves the byte offsets of Sony's param_3 workspace:
///   +0x000: `band_state_low[32]`       per-band flag (nonzero = active)
///   +0x080: `band_peak[32]`            per-band peak signed, flipped
///                                      to negative when processed
///   +0x100: `band_count`               = `*(param_3+0x100)`; loop bound
///   +0x104: `base_bits`                = `*(param_3+0x104)`; header bits
///                                      reserved for tonal payload
///   +0x108: `use_b_path`               = `*(param_3+0x108)` output flag
///   +0x10c: `nonempty`                 = `*(param_3+0x10c)` output flag
///   +0x110: `qmf_band_active[4]`       per-QMF-band activation flag
///   +0x120: `tbl`                      initial 3 (LB always uses tbl=3)
///   +0x124: `count_minus_one`          initial 3 (4 mantissas per entry)
///   +0x128: `cells[16]`                cell blocks, stride 0x20
///   +0x328: `backup_flags[0x86]`       repack-rollback backup of
///                                      +0x110..+0x328 (0x218 bytes)
///   +0x540: `entries[64]`              entry pool, stride 0x18
///   +0xc80: spectrum (separate field — heap-allocated Vec<f32>)
///
/// Sony's fence at `+0x538 + entry_idx*0x18` is the previous entry's
/// "position fence" used in line 43233. In our layout that is
/// `entries[entry_idx - 1].position` (since entry[i-1].sfIndex is at
/// +0x540 + (i-1)*0x18 + 0x14 = +0x554 + (i-1)*0x18, and +0x538 + i*0x18
/// = +0x538 + i*0x18, which for i=1 gives +0x550 = entry[0].position).
/// We compute this relationship explicitly via `prev_entry_position()`.
#[derive(Debug, Clone)]
pub struct SonyLbWorkbench {
    /// +0x000..+0x080 — per-band Phase-4 tbl / low-tier state (i32).
    /// Sony's `piVar12[-0x20]` reads the caller's current tbl index and
    /// may later overwrite it with adjacency offsets.
    pub band_state_low: [i32; 32],
    /// +0x080..+0x100 — per-band Phase-1 peak sfIndex values (i32).
    /// Sony's `*piVar12`, flipped negative once the band has been
    /// processed by FUN_00438ed0.
    pub band_peak: [i32; 32],
    /// +0x100
    pub band_count: i32,
    /// +0x104
    pub base_bits: i32,
    /// +0x108
    pub use_b_path: u32,
    /// +0x10c
    pub nonempty: i32,
    /// +0x110..+0x120 (4 QMF bands).
    pub qmf_band_active: [i32; 4],
    /// +0x120
    pub tbl: i32,
    /// +0x124
    pub count_minus_one: i32,
    /// +0x128..+0x328
    pub cells: [LbCell; 16],
    /// +0x328..+0x540 — 0x218 bytes = 0x86 dwords. Backup buffer for
    /// repack (FUN_00439470 copies +0x110..+0x328 here first and then
    /// restores on repack failure).
    pub backup: [i32; 0x86],
    /// +0x540..+0xc80 — up to 64 entries.
    pub entries: Vec<LbEntry>,
    /// +0xc80..+0x1c80 — spectrum of up to 1024 samples (residual).
    pub spectrum: Vec<f32>,
    /// Sony caller's per-band score array (`auStack_504 + 1`) passed as
    /// param_2 to FUN_00438ed0. Despite older comments this is not an
    /// energy proxy; line 43221 compares the Phase-4 score against 0x500.
    pub band_score: [i32; 32],
}

impl SonyLbWorkbench {
    /// Constructs a workbench from the residual spectrum and caller-
    /// provided per-band context (band_count, base_bits, band_score,
    /// band_flag_low, band_peak).
    ///
    /// Ghidra 43197-43203: the init loop clears +0x110..+0x328 (0x86 dw)
    /// and sets `tbl=3`, `count_minus_one=3`. All other state is
    /// initialised from the caller.
    pub fn new(
        spectrum: Vec<f32>,
        band_count: i32,
        base_bits: i32,
        band_score: [i32; 32],
        band_state_low: [i32; 32],
        band_peak: [i32; 32],
    ) -> Self {
        Self {
            band_state_low,
            band_peak,
            band_count,
            base_bits,
            use_b_path: 0,
            nonempty: 0,
            qmf_band_active: [0; 4],
            tbl: 3,
            count_minus_one: 3,
            cells: [LbCell::default(); 16],
            backup: [0; 0x86],
            entries: Vec::with_capacity(64),
            spectrum,
            band_score,
        }
    }

    /// Sony-exact constructor. Builds the workbench directly from the
    /// Phase-1/Phase-4 inputs produced by `sony_phase1_lb_inputs` — no
    /// Rust-side approximations of the caller arrays.
    pub fn new_from_phase1(spectrum: Vec<f32>, phase1: &SonyLbPhase1Inputs) -> Self {
        Self::new(
            spectrum,
            phase1.band_count,
            phase1.base_bits,
            phase1.band_score,
            phase1.band_state_low,
            phase1.band_peak,
        )
    }
}

/// Port of Sony FUN_004395b0 (Ghidra 43490-43537).
///
/// After FUN_00438b60 produced an all-empty mantissas tuple (mantissa[0]
/// == 0), FUN_00438ed0 calls this to "leading-zero-shift" the entry:
///   1. Walks mantissas[1..4] to find first non-zero.
///   2. Adds that shift count to the entry position; returns -1 if the
///      shifted position crosses the band end (DAT_0048b524[tbl_row]).
///   3. Shifts mantissas left by the skip count, zero-filling the tail.
///   4. Commits the new position; returns the shift amount.
///
/// `band_end` is `(&DAT_0048b524)[band_row]` — the end-of-band boundary
/// for the originating QMF band row in `SONY_SUBBAND_BOUNDARIES[row+1]`.
pub fn sony_lb_shift_leading_zeros(entry: &mut LbEntry, band_end: i32) -> i32 {
    // Ghidra 43501-43505: find first non-zero mantissa starting at 1.
    let mut shift: i32 = 1;
    while shift < 4 {
        if entry.mantissas[shift as usize] != 0 {
            break;
        }
        shift += 1;
    }

    // Ghidra 43506: new_position = position + shift.
    let new_position = entry.position + shift;

    // Ghidra 43507-43509: if crosses band end → fail with -1.
    if band_end <= new_position {
        return -1;
    }
    // Ghidra 43510-43512: if > 0x3fc → return 0 (no shift applied).
    if 0x3fc < new_position {
        return 0;
    }

    // Ghidra 43513-43521: shift mantissas left.
    let limit = (4 - shift) as usize;
    let mut dst: usize = 0;
    let mut src = shift as usize;
    while dst < limit {
        entry.mantissas[dst] = entry.mantissas[src];
        dst += 1;
        src += 1;
    }

    // Ghidra 43522-43534: zero the freed tail slots. The decompile's
    // switch-like structure clears mantissas[1..=3] depending on how many
    // were shifted in.
    match limit {
        1 => {
            entry.mantissas[1] = 0;
            entry.mantissas[2] = 0;
            entry.mantissas[3] = 0;
        }
        2 => {
            entry.mantissas[2] = 0;
            entry.mantissas[3] = 0;
        }
        3 => {
            entry.mantissas[3] = 0;
        }
        _ => {}
    }

    // Ghidra 43535: commit.
    entry.position = new_position;
    shift
}

/// Port of Sony FUN_004393d0 (Ghidra 43367-43401).
///
/// "Final repack" pass: walks the entries backward and left-shifts their
/// mantissas, incrementing position, until any entry either has a
/// leading non-zero mantissa or hits the `0x3f` position-low-6-bits mask
/// (indicating end of its 64-sample cell). Each pass decrements
/// `count_minus_one`. Returns `3 - final_count_minus_one`, i.e. the
/// number of mantissa-slots trimmed.
pub fn sony_lb_final_repack(wb: &mut SonyLbWorkbench, entry_count: i32) -> i32 {
    let mut b_var1 = false;
    loop {
        // Ghidra 43378: iVar3 = count_minus_one.
        let mut count_m1 = wb.count_minus_one;

        // Ghidra 43379: walk entries backward from entry[param_2-1].
        // piVar2 = param_1 + (param_2*3 + 0xa5)*8 = entry[param_2 - 1].
        let mut entry_idx = entry_count - 1;

        while entry_idx >= 0 {
            let ent = &mut wb.entries[entry_idx as usize];

            // Ghidra 43381-43389: if mantissas[count_m1] != 0, shift
            // mantissas left by one and bump position.
            if ent.mantissas[count_m1 as usize] != 0 {
                ent.position += 1;
                // Shift left.
                let mut i: i32 = 0;
                while i < wb.count_minus_one {
                    ent.mantissas[i as usize] = ent.mantissas[(i + 1) as usize];
                    i += 1;
                }
            }

            // Ghidra 43391-43394: re-read cnt (may have been mutated by
            // the shift loop in Sony's compile, but our shift reads from
            // i+1 bounded by wb.count_minus_one so no mutation). Mark
            // bVar1 true when the trailing mantissa still has content.
            count_m1 = wb.count_minus_one;
            let trailing = ent.mantissas[(count_m1 - 1) as usize];
            if trailing != 0 && (ent.mantissas[0] != 0 || ((ent.position as u32) & 0x3f) == 0x3f) {
                b_var1 = true;
            }

            entry_idx -= 1;
        }

        // Ghidra 43397-43398: decrement count_minus_one.
        let new_count_m1 = wb.count_minus_one - 1;
        wb.count_minus_one = new_count_m1;
        // Ghidra 43399: loop while !bVar1.
        if b_var1 {
            return 3 - new_count_m1;
        }
        // Safety: don't go below zero — Sony relies on bVar1 being set
        // before that ever happens, but defensively clamp.
        if new_count_m1 <= 0 {
            return 3 - new_count_m1;
        }
    }
}

/// Port of Sony FUN_00439470 (Ghidra 43405-43486).
///
/// "Repack / split" pass. The goal: split each entry whose
/// `mantissas[3]` is non-zero into two entries, the new one placed at
/// `position + 2` with the upper two mantissas shifted down. This
/// compresses the common case where only the first half of a 4-sample
/// entry carries energy.
///
/// First snapshots `+0x110..+0x328` (qmf flags + cell blocks) into the
/// `+0x328` backup area. On any failure (cell overflow, position > 0x3fd,
/// entry pool exhausted, qmf band not active in the backup) returns 0
/// meaning "abort and do not use this repacked layout".
///
/// On success, returns the new entry count.
pub fn sony_lb_split_repack(wb: &mut SonyLbWorkbench, mut entry_count: i32) -> i32 {
    // Ghidra 43422-43428: copy +0x110..+0x328 into +0x328..+0x540. The
    // range is exactly 0x86 dwords. Order: qmf flags (4), tbl (1),
    // cnt (1), cells (128). We fold that into a single packed buffer.
    let mut flat: [i32; 0x86] = [0; 0x86];
    flat[0] = wb.qmf_band_active[0];
    flat[1] = wb.qmf_band_active[1];
    flat[2] = wb.qmf_band_active[2];
    flat[3] = wb.qmf_band_active[3];
    flat[4] = wb.tbl;
    flat[5] = wb.count_minus_one;
    for (i, cell) in wb.cells.iter().enumerate() {
        let base = 6 + i * 8;
        flat[base] = cell.count;
        for (j, &r) in cell.entry_refs.iter().enumerate() {
            flat[base + 1 + j] = r as i32;
        }
    }
    wb.backup = flat;

    // Ghidra 43429-43454: first loop — for each existing entry, if its
    // mantissa[3] != 0, validate that splitting it at +2 is possible
    // (qmf band still active in backup, cell count < 7, entry_count != 64,
    // new_pos <= 0x3fd). Also bumps the cell's count in the LIVE state.
    if entry_count > 0 {
        let iv_ar9 = entry_count;
        // piVar8 = param_1 + 0x550 = &entry[0].position. piVar8[-1] =
        // entry[0].mantissas[3]. piVar8 += 6 per iter (next entry).
        let mut param_1_counter: i32 = 0;
        let mut i_idx: usize = 0;
        while param_1_counter < iv_ar9 {
            let ent = wb.entries[i_idx];
            let last_mant = ent.mantissas[3];
            if last_mant != 0 {
                // Ghidra 43434: iVar6 = *piVar8 + 2 = position + 2.
                let new_pos = ent.position + 2;
                // Ghidra 43435-43437: check backup qmf band active.
                let qmf_band_idx = (new_pos >> 8) as usize;
                if qmf_band_idx >= 4 || wb.backup[qmf_band_idx] == 0 {
                    return 0;
                }
                // Ghidra 43438: cell_idx for LIVE cell-count bump.
                let cell_idx = (new_pos >> 6) as usize;
                if cell_idx >= 16 {
                    return 0;
                }
                let cell_count = wb.cells[cell_idx].count;
                // Ghidra 43440-43442: if count > 6 → return 0.
                if 6 < cell_count {
                    return 0;
                }
                // Ghidra 43443-43445: if entry_count == 64 → return 0.
                if entry_count == 0x40 {
                    return 0;
                }
                // Ghidra 43446-43448: if new_pos > 0x3fd → return 0.
                if 0x3fd < new_pos {
                    return 0;
                }
                // Ghidra 43449-43450: bump LIVE cell count, entry_count.
                wb.cells[cell_idx].count = cell_count + 1;
                entry_count += 1;
            }
            // Ghidra 43452-43454: piVar8 += 6, param_1++.
            i_idx += 1;
            param_1_counter += 1;
        }
    }

    // Ghidra 43455-43484: second loop — perform the actual split. Walks
    // original entries 0..iv_ar9, writes new entries at puVar4 (starts at
    // entry[iv_ar9] and advances by 1 only when a split happens).
    let iv_ar9 = {
        // iv_ar9 as it stood BEFORE first loop bumps. Reconstruct:
        // entry_count - (bumps made). But entry_count already includes
        // bumps. Sony's code uses the captured iVar9 at 43420 (the
        // original param_2). We need it too. So we re-derive it by
        // counting splits in the first pass.
        let mut orig_count = entry_count;
        // Undo by counting mantissas[3] != 0 among original entries.
        let splits_in_first = {
            // The first loop iterated over "original" entries, but from
            // an outer perspective the original count is what `entry_count`
            // was BEFORE the first pass. Since we don't store that
            // separately, compute splits from the entry data.
            let mut n = 0;
            for ent in wb.entries.iter() {
                if ent.mantissas[3] != 0 {
                    n += 1;
                }
            }
            n
        };
        orig_count -= splits_in_first as i32;
        orig_count
    };

    // Ghidra 43457-43460: only second loop if iv_ar9 > 0.
    if iv_ar9 > 0 {
        // puVar7 = param_1 + 0x548 = entry[0].mantissas[2] (dword 2).
        // puVar4 = entry[iv_ar9].
        let mut src_idx: i32 = 0;
        let mut dst_idx: i32 = iv_ar9;
        let mut remaining: i32 = iv_ar9;

        while remaining > 0 {
            // Source entry is wb.entries[src_idx]. In Sony, puVar7[0] =
            // entry.mantissas[2], puVar7[1] = mantissas[3], puVar7[2] =
            // position, puVar7[3] = sfIndex.
            //
            // Clone source so we can mutate source entry in-place.
            let src_ent = wb.entries[src_idx as usize];
            let needs_split = src_ent.mantissas[3] != 0;

            if needs_split {
                let dst_pos = src_ent.position + 2;
                let new_entry = LbEntry {
                    mantissas: [src_ent.mantissas[2], src_ent.mantissas[3], 0, 0],
                    position: dst_pos,
                    sf_index: src_ent.sf_index,
                };
                // Ghidra 43464-43471: write new entry, clear upper
                // mantissas in source.
                // Ensure dst slot exists.
                while wb.entries.len() <= dst_idx as usize {
                    wb.entries.push(LbEntry::default());
                }
                wb.entries[dst_idx as usize] = new_entry;
                // Clear source mantissas[2..4].
                wb.entries[src_idx as usize].mantissas[2] = 0;
                wb.entries[src_idx as usize].mantissas[3] = 0;

                // Ghidra 43473-43478: add dst entry to its cell.
                let dst_cell_idx = ((dst_pos >> 6) & 0x1f) as usize;
                let cell = &mut wb.cells[dst_cell_idx];
                // Find next empty slot = count - 1 (since count was pre-
                // incremented in first pass).
                let slot = (cell.count - 1) as usize;
                if slot < 7 {
                    cell.entry_refs[slot] = dst_idx as u32;
                }

                dst_idx += 1;
            }

            src_idx += 1;
            remaining -= 1;
        }
    }

    // Ghidra 43485: return new entry count (= param_2 + splits).
    entry_count
}

/// Port of Sony FUN_00438ed0 (Ghidra 43167-43362).
///
/// Low-budget tonal builder — the 1:1 replacement for the heuristic
/// `extract_low_budget_tonal_components` in `quant_tonal.rs`.
///
/// `budget_bits` = Sony's `param_1 = uStack_51c` (tonal bit budget).
///
/// Returns Sony's raw bit cost:
///   `local_3c + iVar10 + local_48`
///
/// where:
///   - `local_3c` = running header overhead
///   - `iVar10`   = `*(param_3 + 0x104)` base_bits
///   - `local_48` = sum of quantised-entry bit costs
pub fn sony_tonal_build_low_budget(budget_bits: i32, wb: &mut SonyLbWorkbench) -> i32 {
    // -- Ghidra 43196-43203: clear +0x110..+0x328, set tbl=3, cnt=3.
    wb.qmf_band_active = [0; 4];
    wb.use_b_path = 0;
    wb.nonempty = 0;
    wb.cells = [LbCell::default(); 16];
    wb.tbl = 3;
    wb.count_minus_one = 3;
    wb.entries.clear();

    // -- Ghidra 43204-43213: initial locals.
    let mut i_var5: i32 = 0; // entry count (param_3 in Sony loc)
    let mut local_3c: i32 = 8; // running header bits (8 base)
    let mut param_3_local: i32 = 0; // aliased entry count for loop exits
    let mut local_44: i32 = 0; // "row" parameter fed into DAT lookups
    let mut local_48: i32 = 0; // running cost total
    let mut local_40: i32 = 0; // count of entries with mantissa[0]==0
    let mut i_var10: i32 = 0; // band iterator (Sony's initial value)

    // If budget < 0x44c (1100), skip the first 8 bands.
    if budget_bits < 0x44c {
        local_44 = 8;
        i_var10 = 8;
    }

    // -- Ghidra 43216-43310: main per-band loop.
    'band_loop: loop {
        if i_var10 >= wb.band_count {
            break;
        }

        // Ghidra 43217-43218: two termination tests.
        //   (a) `0x40 - (weight >> 4) < entry_count` — Sony's active-count
        //       weight limit for this band. At band 0 weight=8, shift=0
        //       so limit=0x40; at higher bands the limit tightens.
        //   (b) `(budget - 600) < entry_count*0x18 + local_3c` — soft
        //       budget bound with per-entry bit estimate.
        let weight = SONY_ACTIVE_COUNT_WEIGHT[i_var10 as usize];
        let active_limit = 0x40 - (weight >> 4);
        if active_limit < i_var5 {
            break;
        }
        let i_var14 = i_var5 * 0x18;
        if (budget_bits - 600) < i_var14 + local_3c {
            break;
        }

        // Ghidra 43219: i_var7 = band_state_low[band] (the flag).
        let mut i_var7 = wb.band_state_low[i_var10 as usize];
        if i_var7 != 0 {
            // Ghidra 43221-43223: clamp the incoming tbl/flag to 0 when
            // the caller's Phase-4 score (`auStack_504[band+1]`) is below
            // 0x500.
            if wb.band_score[i_var10 as usize] < 0x500 {
                i_var7 = 0;
            }
            // Ghidra 43224-43228: derive DAT_0048ce70 row index.
            // iVar3 = DAT_0048b520[band] (subband start), iVar11 = iVar3 >> 8.
            // If ACTIVE_COUNT_WEIGHT[band] == 0x20, iVar11 = 1.
            let i_var3_band_start = SONY_SUBBAND_BOUNDARIES[i_var10 as usize] as i32;
            let mut i_var11: i32 = i_var3_band_start >> 8;
            if weight == 0x20 {
                i_var11 = 1;
            }
            // Ghidra 43229-43232: scan start position = band_peak[band] -
            // DAT_0048ce70[i_var7 + i_var11*8]. Negative becomes 0.
            let bias_idx = (i_var7 + i_var11 * 8) as usize;
            let spacing_bias = if bias_idx < SONY_LOW_BUDGET_SPACING_BIAS.len() {
                SONY_LOW_BUDGET_SPACING_BIAS[bias_idx]
            } else {
                0
            };
            let mut scan_start: i32 = wb.band_peak[i_var10 as usize] - spacing_bias;
            if scan_start < 0 {
                scan_start = 0;
            }
            // `iVar3` is reused as scan_end just below.
            let mut scan_end: i32 = i_var3_band_start;

            // Ghidra 43233-43239: adjacency clamp. If iVar5 > 0 and the
            // previous entry's position is too close (band_start <
            // prev_entry.position + 4), flip the band_peak sign and
            // extend the scan.
            if i_var5 > 0 {
                let prev_entry_pos = wb.entries[(i_var5 - 1) as usize].position;
                if i_var3_band_start < prev_entry_pos + 4 {
                    // Sony: *piVar12 = -*piVar12 (flip this band's peak).
                    wb.band_peak[i_var10 as usize] = -wb.band_peak[i_var10 as usize];
                    // Sony: piVar12[-0x20] = iVar3 + 4 (update flag to a
                    // positive value indicating new offset).
                    let new_flag = (prev_entry_pos - i_var3_band_start) + 4;
                    wb.band_state_low[i_var10 as usize] = new_flag;
                    // Sony: piVar12[-0x21] = piVar12[-0x21] + iVar3.
                    // This is the PREVIOUS band's flag (band-1), updated
                    // with the offset.
                    if i_var10 > 0 {
                        wb.band_state_low[(i_var10 - 1) as usize] +=
                            prev_entry_pos - i_var3_band_start;
                    }
                    // iVar3 (scan_end) updated.
                    scan_end = prev_entry_pos + 4;
                }
            }

            // Ghidra 43240-43241: FUN_00439640 — scan for tonal
            // positions. Returns number of candidates written into
            // local_24[0..9].
            // `(&DAT_0048b524)[local_44]` = subband end for row local_44.
            let band_end_for_scan = SONY_SUBBAND_BOUNDARIES[(local_44 + 1) as usize];
            let class_idx = ((SONY_ACTIVE_COUNT_WEIGHT[local_44 as usize] + 8) >> 4) as usize;
            let mut local_24: [i32; 10] = [0; 10];
            // Sony passes `scan_start` as both param_1 (start) and iVar3
            // (= scan_end). The call is:
            //   FUN_00439640(param_2, iVar3, (&DAT_0048b524)[local_44],
            //                (&DAT_0048cc10)[local_44]+8 >> 4,
            //                (int)local_24, iVar2);
            //
            // where `param_2` = scan_start (written to "0" earlier) and
            // iVar3 = scan_end. So scan range is [param_2..iVar3).
            //
            // Our port `sony_lb_scan_positions` is a direct port of
            // FUN_00439640. Map args:
            //   class_idx       = (weight[row] + 8) >> 4 → PROMINENCE class
            //   start_pos       = scan_start as usize
            //   end_pos         = band_end_for_scan
            //   max_candidates  = 9
            //   out_positions   = &mut local_24[..]
            //   residual        = wb.spectrum[..]
            let i_var7 = sony_lb_scan_positions(
                class_idx.min(SONY_TONAL_PROMINENCE_THRESHOLDS.len() - 1),
                scan_start.max(0) as usize,
                band_end_for_scan,
                9,
                &mut local_24,
                &wb.spectrum,
            );

            // Ghidra 43242-43243: restore loop invariants.
            i_var5 = param_3_local;
            i_var10 = local_44;

            if 0 < i_var7 {
                // Ghidra 43245-43248: if band_peak >= 0, flip it negative
                // and clear flag. This marks the band as "processed".
                if 0 <= wb.band_peak[i_var10 as usize] {
                    wb.band_peak[i_var10 as usize] = -wb.band_peak[i_var10 as usize];
                    wb.band_state_low[i_var10 as usize] = 0;
                }

                // Ghidra 43249-43252: set sentinel, init local_38 =
                // entries[param_3_local].
                local_24[i_var7 as usize] = -1;

                let _band_end_fence = scan_end + 4; // "param_2 = +0x538 row"

                // Ghidra 43253-43304: per-candidate inner loop.
                let mut cand_idx: usize = 0;
                loop {
                    let mut cand_pos = local_24[cand_idx];
                    if cand_pos < 0 {
                        break; // sentinel
                    }

                    // Ghidra 43255-43257: clamp to 0x3fc.
                    if 0x3fc < cand_pos {
                        cand_pos = 0x3fc;
                    }
                    let mut i_var14_inner: i32 = 4; // count increment

                    // Ghidra 43258-43267: if iVar5>0 AND
                    // prev_entry_fence+4 == cand_pos, backtrack cand_pos
                    // through descending-magnitude samples (max 4 steps).
                    if i_var5 > 0 {
                        let prev_pos = wb.entries[(i_var5 - 1) as usize].position;
                        if prev_pos + 4 == cand_pos {
                            // Walk backward while |sample[cand_pos-1]| >
                            // |sample[cand_pos]|, max 3 more steps.
                            let mut remaining = 4;
                            while remaining > 1 {
                                let cur = wb
                                    .spectrum
                                    .get(cand_pos as usize)
                                    .copied()
                                    .unwrap_or(0.0)
                                    .abs();
                                let prev_s = if cand_pos > 0 {
                                    wb.spectrum
                                        .get((cand_pos - 1) as usize)
                                        .copied()
                                        .unwrap_or(0.0)
                                        .abs()
                                } else {
                                    0.0
                                };
                                if prev_s < cur {
                                    break;
                                }
                                cand_pos -= 1;
                                remaining -= 1;
                                i_var14_inner -= 1;
                            }
                        }
                    }

                    // Ghidra 43268: iVar3 = cand_pos >> 6 (cell index).
                    let mut cell_idx_for_write = (cand_pos >> 6) as usize;

                    // Ghidra 43269-43272: if cell count < 7.
                    if wb.cells[cell_idx_for_write].count < 7 {
                        // Ghidra 43273-43274: set entry position, run
                        // FUN_00438b60 (quantise+subtract) on
                        // spectrum[cand_pos..cand_pos+4].
                        let entry_idx = i_var5 as usize;
                        // Make sure entry slot exists.
                        while wb.entries.len() <= entry_idx {
                            wb.entries.push(LbEntry::default());
                        }
                        wb.entries[entry_idx].position = cand_pos;
                        wb.entries[entry_idx].mantissas = [0; 4];
                        wb.entries[entry_idx].sf_index = 0;

                        let start = cand_pos as usize;
                        let end = (start + 4).min(wb.spectrum.len());
                        let count = end - start;

                        let tbl_u8: u8 = wb.tbl as u8;
                        let mut mant: [u32; 4] = [0; 4];
                        let mut sf_u8: u8 = 0;
                        let entry_cost = sony_tonal_quantize_and_subtract(
                            &mut wb.spectrum[start..end],
                            &mut mant,
                            &mut sf_u8,
                            count,
                            tbl_u8,
                        );
                        wb.entries[entry_idx].mantissas = mant;
                        wb.entries[entry_idx].sf_index = sf_u8 as i32;

                        let mut used_cost = entry_cost;

                        // Ghidra 43275-43283: empty-quant rescue. If
                        // mantissas[0] == 0 AND position < 0x3fd, call
                        // FUN_004395b0 to left-shift. If that returns -1,
                        // restore via FUN_00438e30 and break out.
                        if wb.entries[entry_idx].mantissas[0] == 0
                            && wb.entries[entry_idx].position < 0x3fd
                        {
                            let band_end_for_shift =
                                SONY_SUBBAND_BOUNDARIES[(local_44 + 1) as usize] as i32;
                            let ent_copy = wb.entries[entry_idx];
                            let shift_result = {
                                let ent = &mut wb.entries[entry_idx];
                                sony_lb_shift_leading_zeros(ent, band_end_for_shift)
                            };
                            if shift_result < 0 {
                                // Restore via FUN_00438e30.
                                let mants_i32: [i32; 4] = [
                                    ent_copy.mantissas[0] as i32,
                                    ent_copy.mantissas[1] as i32,
                                    ent_copy.mantissas[2] as i32,
                                    ent_copy.mantissas[3] as i32,
                                ];
                                sony_tonal_dequant_add(
                                    &mants_i32,
                                    ent_copy.position as usize,
                                    ent_copy.sf_index as u8,
                                    3,
                                    tbl_u8,
                                    &mut wb.spectrum,
                                );
                                i_var5 = param_3_local;
                                break;
                            }
                            // Update cell_idx after shift.
                            cell_idx_for_write = (wb.entries[entry_idx].position >> 6) as usize;
                        }

                        // Ghidra 43284: local_48 += entry_cost.
                        local_48 += used_cost;
                        // Ghidra 43285: band_state_low[band] += i_var14.
                        wb.band_state_low[i_var10 as usize] += i_var14_inner;

                        // Ghidra 43286-43289: activate qmf band (12 bit
                        // cost per newly-activated band).
                        let qmf_band_idx = (cell_idx_for_write >> 2) & 0x3;
                        if wb.qmf_band_active[qmf_band_idx] == 0 {
                            wb.qmf_band_active[qmf_band_idx] = 1;
                            local_3c += 0xc;
                        }

                        // Ghidra 43290-43293: store entry index in cell.
                        if cell_idx_for_write < 16 {
                            let cell = &mut wb.cells[cell_idx_for_write];
                            let slot = cell.count as usize;
                            if slot < 7 {
                                cell.entry_refs[slot] = entry_idx as u32;
                            }
                            cell.count += 1;
                        }

                        // Ghidra 43294-43300: if mantissas[0] == 0 after
                        // (post-shift) quant, bump local_40.
                        if wb.entries[entry_idx].mantissas[0] == 0 {
                            local_40 += 1;
                        }
                        param_3_local += 1;
                        i_var5 = param_3_local;

                        // local_38 += 6 dwords = next entry slot happens
                        // automatically via our Vec.
                        let _ = used_cost; // silence
                        used_cost = 0;
                        let _ = used_cost;
                    }

                    cand_idx += 1;
                    if cand_idx >= local_24.len() {
                        break;
                    }
                }
            }
        }

        // Ghidra 43307-43309: advance band.
        i_var10 += 1;
        local_44 = i_var10;
        // piVar12 increments too (we access band_peak[i_var10] each loop).

        if i_var10 >= wb.band_count {
            break 'band_loop;
        }
    }

    // Ghidra 43311-43316: early-exit cases.
    if i_var5 == 0 {
        return 0;
    }
    if i_var5 == 1 {
        // If first entry's position < 0x80 OR budget < 0x44c → abandon.
        let entry0_pos = wb.entries[0].position;
        if entry0_pos < 0x80 || budget_bits < 0x44c {
            let mants_i32: [i32; 4] = [
                wb.entries[0].mantissas[0] as i32,
                wb.entries[0].mantissas[1] as i32,
                wb.entries[0].mantissas[2] as i32,
                wb.entries[0].mantissas[3] as i32,
            ];
            sony_tonal_dequant_add(
                &mants_i32,
                wb.entries[0].position as usize,
                wb.entries[0].sf_index as u8,
                3,
                wb.tbl as u8,
                &mut wb.spectrum,
            );
            return 0;
        }
    }

    // -- Ghidra 43318-43320: post-pass cost compare / repack gate.
    let i_var10_base_bits = wb.base_bits;
    let mut i_var14 = i_var5 * 0x18;
    wb.nonempty = 1;

    let mut local_48_final = local_48;

    if local_40 != i_var5 {
        // Ghidra 43321-43335: cost evaluation gate.
        let i_var11 = i_var5 * 2 + local_40 * -3;
        let i_var3 = (i_var5 - local_40) * 0xc;

        let sep_unit = SONY_LB_SEP_UNIT_BITS[wb.tbl as usize];
        let mag_unit = SONY_LB_MAG_UNIT_BITS[wb.tbl as usize];

        // iVar15 = sep_unit*iVar11 + iVar3 + local_48
        let i_var15 = sep_unit * i_var11 + i_var3 + local_48;
        // iVar3 = mag_unit*iVar11 + iVar3 + iVar14
        let i_var3_cost = mag_unit * i_var11 + i_var3 + i_var14;

        let i_var7 = i_var14.min(local_48);
        let i_var11_cost = i_var3_cost.min(i_var15);

        if i_var11_cost < i_var7 {
            // Ghidra 43337-43346: FUN_00439470 split-repack. If it
            // returns 0, fall through to the regular path; if >0, use
            // the new entry count and adjust costs.
            let new_entry_count = sony_lb_split_repack(wb, i_var5);
            if new_entry_count != 0 {
                let sep_unit = SONY_LB_SEP_UNIT_BITS[wb.tbl as usize];
                let mag_unit = SONY_LB_MAG_UNIT_BITS[wb.tbl as usize];
                let delta = new_entry_count; // == iVar5 in the decompile context
                local_48_final = sep_unit * delta + i_var15;
                i_var14 = mag_unit * delta + i_var3_cost;
                param_3_local = new_entry_count;
                // Ghidra 43343: goto LAB_0043935b — final_repack.
                let i_var5_final = sony_lb_final_repack(wb, new_entry_count);
                let sep_unit = SONY_LB_SEP_UNIT_BITS[wb.tbl as usize];
                let mag_unit = SONY_LB_MAG_UNIT_BITS[wb.tbl as usize];
                local_48_final -= sep_unit * i_var5_final * param_3_local;
                i_var14 -= mag_unit * i_var5_final * param_3_local;
            } else {
                // Ghidra 43345-43347: reset local_40 to 0.
                local_40 = 0;
                i_var5 = param_3_local;
            }
        }

        if local_40 != i_var5 {
            // Ghidra 43348: goto LAB_00439397 (skip final_repack).
            let final_48 = local_48_final;
            let final_sel = i_var14.min(final_48);
            wb.use_b_path = (i_var14 < final_48) as u32;
            return local_3c + i_var10_base_bits + final_sel;
        }
    }

    // LAB_0043935b: Ghidra 43351-43355 — unconditional final_repack for
    // the "local_40 == i_var5" path.
    if i_var5 > 0 {
        let i_var5_final = sony_lb_final_repack(wb, i_var5);
        let sep_unit = SONY_LB_SEP_UNIT_BITS[wb.tbl as usize];
        let mag_unit = SONY_LB_MAG_UNIT_BITS[wb.tbl as usize];
        local_48_final -= sep_unit * i_var5_final * i_var5;
        i_var14 -= mag_unit * i_var5_final * i_var5;
    }

    // LAB_00439397: Ghidra 43357-43362 — select cheaper path.
    let final_48 = local_48_final;
    let final_sel = i_var14.min(final_48);
    wb.use_b_path = (i_var14 < final_48) as u32;
    local_3c + i_var10_base_bits + final_sel
}
