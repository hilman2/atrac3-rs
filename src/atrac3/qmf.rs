use anyhow::{Result, bail, ensure};
use std::sync::OnceLock;

use super::{QMF_BANDS, SAMPLES_PER_BAND, SAMPLES_PER_FRAME};

const DIRECT_QMF_STATE_SAMPLES: usize = 138;
const DIRECT_QMF_STAGE2_HISTORY_SAMPLES: usize = 132;
const DIRECT_QMF_SCRATCH_SAMPLES: usize = SAMPLES_PER_FRAME + DIRECT_QMF_STATE_SAMPLES;
const DIRECT_QMF_SAMPLE_STRIDE: usize = 4;
const DIRECT_QMF_INSERT_OFFSET: usize = 6;
const DEFAULT_DIRECT_QMF_OUTPUT_GAIN: f32 = 4.0;

const STAGE1_EVEN_COEFF_ORDER: [usize; 24] = [
    46, 2, 6, 10, 14, 18, 22, 26, 30, 34, 38, 42, 44, 0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40,
];
const STAGE1_ODD_COEFF_ORDER: [usize; 24] = [
    47, 3, 7, 11, 15, 19, 23, 27, 31, 35, 39, 43, 45, 1, 5, 9, 13, 17, 21, 25, 29, 33, 37, 41,
];
const STAGE1_PHASE0_OFFSETS: [isize; 24] = [
    6, -38, -34, -30, -26, -22, -18, -14, -10, -6, -2, 2, 4, -40, -36, -32, -28, -24, -20, -16,
    -12, -8, -4, 0,
];
const STAGE1_PHASE1_OFFSETS: [isize; 24] = [
    7, -37, -33, -29, -25, -21, -17, -13, -9, -5, -1, 3, 5, -39, -35, -31, -27, -23, -19, -15, -11,
    -7, -3, 1,
];
const STAGE1_PHASE2_OFFSETS: [isize; 24] = [
    8, -36, -32, -28, -24, -20, -16, -12, -8, -4, 0, 4, 6, -38, -34, -30, -26, -22, -18, -14, -10,
    -6, -2, 2,
];
const STAGE1_PHASE3_OFFSETS: [isize; 24] = [
    9, -35, -31, -27, -23, -19, -15, -11, -7, -3, 1, 5, 7, -37, -33, -29, -25, -21, -17, -13, -9,
    -5, -1, 3,
];
const STAGE2_SIGN_MASKS: [f32; 4] = [-0.0, 0.0, -0.0, 0.0];

pub const LEGACY_QMF_48TAP_HALF: [f32; 24] = [
    -0.000_014_619_07,
    -0.000_092_054_79,
    -0.000_056_157_57,
    0.000_301_172_69,
    0.000_242_251_9,
    -0.000_852_938_97,
    -0.000_520_557_4,
    0.002_034_016_9,
    0.000_783_338_9,
    -0.004_215_386_2,
    -0.000_756_149_9,
    0.007_840_294_,
    -0.000_061_169_92,
    -0.013_441_62,
    0.002_462_682,
    0.021_736_089,
    -0.007_801_671,
    -0.034_090_22,
    0.018_809_49,
    0.054_326_01,
    -0.043_596_38,
    -0.099_384_37,
    0.132_079_1,
    0.464_241_6,
];

pub const EXE_QMF_48TAP_HALF: [f32; 24] = [
    0.000_015_258_789,
    0.000_096_083_04,
    0.000_058_614_98,
    -0.000_314_351_78,
    -0.000_252_852_66,
    0.000_890_262_95,
    0.000_543_336_56,
    -0.002_123_024,
    -0.000_817_617_16,
    0.004_399_848,
    0.000_789_238_43,
    -0.008_183_379,
    0.000_063_846_67,
    0.014_029_815,
    -0.002_570_447,
    -0.022_687_245,
    0.008_143_067,
    0.035_581_98,
    -0.019_632_578,
    -0.056_703_273,
    0.045_504_123,
    0.103_733_35,
    -0.137_858_78,
    -0.484_556_44,
];

fn qmf_use_direct_analysis() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_QMF_DIRECT")
            .ok()
            .map(|value| {
                !matches!(
                    value.to_ascii_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                )
            })
            .unwrap_or(true)
    })
}

fn direct_qmf_output_gain() -> f32 {
    static VALUE: OnceLock<f32> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_QMF_DIRECT_GAIN")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(DEFAULT_DIRECT_QMF_OUTPUT_GAIN)
    })
}

fn qmf_use_exe_window() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_QMF_EXE_WINDOW")
            .ok()
            .map(|value| {
                !matches!(
                    value.to_ascii_lowercase().as_str(),
                    "0" | "false" | "no" | "off"
                )
            })
            .unwrap_or(true)
    })
}

fn qmf_high_is_odd_minus_even() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_QMF_HIGH_ODD_MINUS_EVEN")
            .ok()
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(true)
    })
}

fn qmf_root_swap() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_QMF_ROOT_SWAP")
            .ok()
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    })
}

fn qmf_upper_swap() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_QMF_UPPER_SWAP")
            .ok()
            .map(|value| {
                matches!(
                    value.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(true)
    })
}

pub fn mirrored_qmf_window() -> [f32; 48] {
    let mut out = [0.0; 48];
    let half = if qmf_use_exe_window() {
        EXE_QMF_48TAP_HALF
    } else {
        LEGACY_QMF_48TAP_HALF
    };
    for (i, coeff) in half.into_iter().enumerate() {
        let v = coeff * 2.0;
        out[i] = v;
        out[47 - i] = v;
    }
    out
}

fn exe_qmf_window() -> [f32; 48] {
    let mut out = [0.0f32; 48];
    for (i, coeff) in EXE_QMF_48TAP_HALF.into_iter().enumerate() {
        out[i] = coeff;
        out[47 - i] = coeff;
    }
    out
}

#[derive(Debug, Clone)]
pub struct TwoBandQmf {
    delay: [f32; 46],
    window: [f32; 48],
}

impl Default for TwoBandQmf {
    fn default() -> Self {
        Self {
            delay: [0.0; 46],
            window: mirrored_qmf_window(),
        }
    }
}

impl TwoBandQmf {
    pub fn split_block(&mut self, input: &[f32]) -> Result<(Vec<f32>, Vec<f32>)> {
        ensure!(
            input.len() & 1 == 0,
            "QMF input must contain an even number of samples"
        );

        let mut history = Vec::with_capacity(self.delay.len() + input.len());
        history.extend_from_slice(&self.delay);
        history.extend_from_slice(input);

        let out_len = input.len() / 2;
        let mut low = vec![0.0; out_len];
        let mut high = vec![0.0; out_len];

        for k in 0..out_len {
            let base = k * 2;
            let mut even_acc = 0.0f32;
            let mut odd_acc = 0.0f32;

            for tap in 0..24 {
                even_acc += history[base + tap * 2] * self.window[tap * 2];
                odd_acc += history[base + tap * 2 + 1] * self.window[tap * 2 + 1];
            }

            low[k] = even_acc + odd_acc;
            high[k] = if qmf_high_is_odd_minus_even() {
                odd_acc - even_acc
            } else {
                even_acc - odd_acc
            };
        }

        let delay_len = self.delay.len();
        self.delay
            .copy_from_slice(&history[history.len() - delay_len..]);

        Ok((low, high))
    }
}

#[derive(Debug, Clone)]
pub struct FourBandQmf {
    root: TwoBandQmf,
    low_split: TwoBandQmf,
    high_split: TwoBandQmf,
    direct_state: [f32; DIRECT_QMF_STATE_SAMPLES],
}

#[derive(Debug, Clone)]
pub struct FourBandFrame {
    pub bands: [Vec<f32>; QMF_BANDS],
    pub interleaved: [f32; SAMPLES_PER_FRAME],
}

impl Default for FourBandQmf {
    fn default() -> Self {
        Self {
            root: TwoBandQmf::default(),
            low_split: TwoBandQmf::default(),
            high_split: TwoBandQmf::default(),
            direct_state: [0.0; DIRECT_QMF_STATE_SAMPLES],
        }
    }
}

impl FourBandQmf {
    pub fn split_frame(&mut self, frame: &[f32]) -> Result<[Vec<f32>; QMF_BANDS]> {
        Ok(self.split_frame_with_layout(frame)?.bands)
    }

    pub fn split_frame_with_layout(&mut self, frame: &[f32]) -> Result<FourBandFrame> {
        if frame.len() != SAMPLES_PER_FRAME {
            bail!(
                "ATRAC3 4-band analysis expects {} samples, got {}",
                SAMPLES_PER_FRAME,
                frame.len()
            );
        }

        if qmf_use_direct_analysis() {
            return self.split_frame_direct(frame);
        }

        let (root_first, root_second) = self.root.split_block(frame)?;
        let (low_half, high_half) = if qmf_root_swap() {
            (root_second, root_first)
        } else {
            (root_first, root_second)
        };
        let (band0, band1) = self.low_split.split_block(&low_half)?;
        let (upper_first, upper_second) = self.high_split.split_block(&high_half)?;
        let (band2, band3) = if qmf_upper_swap() {
            (upper_second, upper_first)
        } else {
            (upper_first, upper_second)
        };

        debug_assert_eq!(band0.len(), SAMPLES_PER_BAND);
        debug_assert_eq!(band1.len(), SAMPLES_PER_BAND);
        debug_assert_eq!(band2.len(), SAMPLES_PER_BAND);
        debug_assert_eq!(band3.len(), SAMPLES_PER_BAND);

        let bands = [band0, band1, band2, band3];
        let interleaved = interleave_bands(&bands);
        Ok(FourBandFrame { bands, interleaved })
    }

    fn split_frame_direct(&mut self, frame: &[f32]) -> Result<FourBandFrame> {
        ensure!(
            frame.len() == SAMPLES_PER_FRAME,
            "ATRAC3 direct QMF expects {} samples, got {}",
            SAMPLES_PER_FRAME,
            frame.len()
        );

        let window = exe_qmf_window();
        let stage2 = direct_stage2_coefficients();
        let output_gain = direct_qmf_output_gain();
        let mut scratch = [0.0f32; DIRECT_QMF_SCRATCH_SAMPLES];
        scratch[..DIRECT_QMF_STATE_SAMPLES].copy_from_slice(&self.direct_state);

        let mut interleaved = [0.0f32; SAMPLES_PER_FRAME];
        let mut cursor = DIRECT_QMF_STAGE2_HISTORY_SAMPLES;

        for sample_index in 0..SAMPLES_PER_BAND {
            let input_base = sample_index * DIRECT_QMF_SAMPLE_STRIDE;
            scratch[cursor + DIRECT_QMF_INSERT_OFFSET] = frame[input_base];
            scratch[cursor + DIRECT_QMF_INSERT_OFFSET + 1] = frame[input_base + 1];
            scratch[cursor + DIRECT_QMF_INSERT_OFFSET + 2] = frame[input_base + 2];
            scratch[cursor + DIRECT_QMF_INSERT_OFFSET + 3] = frame[input_base + 3];

            let phase0 = direct_dot(
                &scratch,
                cursor,
                &window,
                &STAGE1_EVEN_COEFF_ORDER,
                &STAGE1_PHASE0_OFFSETS,
            );
            let phase1 = direct_dot(
                &scratch,
                cursor,
                &window,
                &STAGE1_ODD_COEFF_ORDER,
                &STAGE1_PHASE1_OFFSETS,
            );
            let phase2 = direct_dot(
                &scratch,
                cursor,
                &window,
                &STAGE1_EVEN_COEFF_ORDER,
                &STAGE1_PHASE2_OFFSETS,
            );
            let phase3 = direct_dot(
                &scratch,
                cursor,
                &window,
                &STAGE1_ODD_COEFF_ORDER,
                &STAGE1_PHASE3_OFFSETS,
            );

            let stage1_a = phase0 + phase1;
            let stage1_b = phase2 + phase3;
            let stage1_c = phase1 + (-phase0);
            let stage1_d = phase3 + (-phase2);

            scratch[cursor - 40] = stage1_a;
            scratch[cursor - 39] = stage1_b;
            scratch[cursor - 38] = stage1_c;
            scratch[cursor - 37] = stage1_d;

            let band_mix0 = direct_stage2_dot(&scratch, cursor, stage1_a, 0, &stage2);
            let band_mix1 = direct_stage2_dot(&scratch, cursor, stage1_b, 1, &stage2);
            let band_mix2 = direct_stage2_dot(&scratch, cursor, stage1_c, 2, &stage2);
            let band_mix3 = direct_stage2_dot(&scratch, cursor, stage1_d, 3, &stage2);

            interleaved[input_base] =
                (band_mix0 + apply_sign_mask(band_mix1, STAGE2_SIGN_MASKS[1])) * output_gain;
            interleaved[input_base + 1] =
                (band_mix1 + apply_sign_mask(band_mix0, STAGE2_SIGN_MASKS[0])) * output_gain;
            interleaved[input_base + 2] =
                (band_mix3 + apply_sign_mask(band_mix2, STAGE2_SIGN_MASKS[2])) * output_gain;
            interleaved[input_base + 3] =
                (band_mix2 + apply_sign_mask(band_mix3, STAGE2_SIGN_MASKS[3])) * output_gain;

            cursor += DIRECT_QMF_SAMPLE_STRIDE;
        }

        self.direct_state
            .copy_from_slice(&scratch[SAMPLES_PER_FRAME..]);

        let mut band0 = vec![0.0f32; SAMPLES_PER_BAND];
        let mut band1 = vec![0.0f32; SAMPLES_PER_BAND];
        let mut band2 = vec![0.0f32; SAMPLES_PER_BAND];
        let mut band3 = vec![0.0f32; SAMPLES_PER_BAND];
        for sample_index in 0..SAMPLES_PER_BAND {
            let base = sample_index * DIRECT_QMF_SAMPLE_STRIDE;
            band0[sample_index] = interleaved[base];
            band1[sample_index] = interleaved[base + 1];
            band2[sample_index] = interleaved[base + 2];
            band3[sample_index] = interleaved[base + 3];
        }

        Ok(FourBandFrame {
            bands: [band0, band1, band2, band3],
            interleaved,
        })
    }
}

pub fn estimate_envelopes_from_interleaved(
    interleaved: &[f32; SAMPLES_PER_FRAME],
) -> [[f32; SAMPLES_PER_BAND / 8]; QMF_BANDS] {
    let mut out = [[0.0f32; SAMPLES_PER_BAND / 8]; QMF_BANDS];
    for slot_index in 0..(SAMPLES_PER_BAND / 8) {
        let slot_base = slot_index * 8 * QMF_BANDS;
        for sample_offset in 0..8 {
            let sample_base = slot_base + sample_offset * QMF_BANDS;
            for band_index in 0..QMF_BANDS {
                out[band_index][slot_index] =
                    out[band_index][slot_index].max(interleaved[sample_base + band_index].abs());
            }
        }
    }
    out
}

fn interleave_bands(bands: &[Vec<f32>; QMF_BANDS]) -> [f32; SAMPLES_PER_FRAME] {
    let mut out = [0.0f32; SAMPLES_PER_FRAME];
    for sample_index in 0..SAMPLES_PER_BAND {
        let base = sample_index * QMF_BANDS;
        out[base] = bands[0][sample_index];
        out[base + 1] = bands[1][sample_index];
        out[base + 2] = bands[2][sample_index];
        out[base + 3] = bands[3][sample_index];
    }
    out
}

fn direct_dot(
    scratch: &[f32; DIRECT_QMF_SCRATCH_SAMPLES],
    cursor: usize,
    coeffs: &[f32; 48],
    coeff_order: &[usize; 24],
    offsets: &[isize; 24],
) -> f32 {
    coeff_order
        .iter()
        .zip(offsets.iter())
        .map(|(coeff_index, offset)| {
            coeffs[*coeff_index] * scratch[(cursor as isize + offset) as usize]
        })
        .sum()
}

fn direct_stage2_coefficients() -> [[f32; 24]; 4] {
    let full = exe_qmf_window();
    let mut out = [[0.0f32; 24]; 4];
    for lane in 0..4 {
        let parity = lane & 1;
        out[lane][0] = full[46 + parity];
        for tap in 0..23 {
            out[lane][tap + 1] = full[parity + tap * 2];
        }
    }
    out
}

fn direct_stage2_dot(
    scratch: &[f32; DIRECT_QMF_SCRATCH_SAMPLES],
    cursor: usize,
    current: f32,
    lane: usize,
    coeffs: &[[f32; 24]; 4],
) -> f32 {
    let mut acc = coeffs[lane][0] * current;
    let lane_offset = lane as isize;
    for tap in 0..23 {
        let offset = cursor as isize - DIRECT_QMF_STAGE2_HISTORY_SAMPLES as isize
            + lane_offset
            + tap as isize * DIRECT_QMF_SAMPLE_STRIDE as isize;
        acc += coeffs[lane][tap + 1] * scratch[offset as usize];
    }
    acc
}

fn apply_sign_mask(value: f32, sign: f32) -> f32 {
    f32::from_bits(value.to_bits() ^ sign.to_bits())
}

#[cfg(test)]
mod tests {
    use super::{FourBandQmf, estimate_envelopes_from_interleaved, mirrored_qmf_window};
    use crate::atrac3::gain::estimate_envelope_slots;

    #[test]
    fn mirrors_window() {
        let window = mirrored_qmf_window();
        assert_eq!(window[0], window[47]);
        assert_eq!(window[1], window[46]);
    }

    #[test]
    fn splits_to_four_bands() {
        let mut qmf = FourBandQmf::default();
        let frame = vec![0.0f32; 1024];
        let bands = qmf.split_frame(&frame).unwrap();

        assert_eq!(bands.len(), 4);
        assert_eq!(bands[0].len(), 256);
    }

    #[test]
    fn interleaved_envelope_matches_planar_chunks() {
        let mut frame = [0.0f32; 1024];
        for (index, sample) in frame.iter_mut().enumerate() {
            *sample = ((index as f32 * 0.03125).sin() * (1.0 + (index % 11) as f32 * 0.05))
                + ((index as f32 * 0.0078125).cos() * 0.25);
        }

        let mut qmf = FourBandQmf::default();
        let split = qmf.split_frame_with_layout(&frame).unwrap();
        let interleaved = estimate_envelopes_from_interleaved(&split.interleaved);

        for (band_index, band) in split.bands.iter().enumerate() {
            let planar = estimate_envelope_slots(band).unwrap();
            assert_eq!(interleaved[band_index], planar);
        }
    }
}
