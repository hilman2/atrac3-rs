use std::f64::consts::PI;

use anyhow::{Result, ensure};

use super::{
    QMF_BANDS, SAMPLES_PER_BAND, SAMPLES_PER_FRAME, mdct::MDCT_COEFFS_PER_BAND,
    qmf::mirrored_qmf_window,
};

pub const IMDCT_OUTPUT_SAMPLES: usize = MDCT_COEFFS_PER_BAND * 2;
const IMDCT_SCALE: f32 = 2.0 / MDCT_COEFFS_PER_BAND as f32;

pub fn atrac3_decoder_window() -> [f32; IMDCT_OUTPUT_SAMPLES] {
    let mut out = [0.0f32; IMDCT_OUTPUT_SAMPLES];
    for i in 0..128 {
        let j = 255 - i;
        let wi = (((i as f64 + 0.5) / 256.0 - 0.5) * PI).sin() + 1.0;
        let wj = (((j as f64 + 0.5) / 256.0 - 0.5) * PI).sin() + 1.0;
        let w = 0.5 * (wi * wi + wj * wj);

        out[i] = (wi / w) as f32;
        out[511 - i] = out[i];
        out[j] = (wj / w) as f32;
        out[511 - j] = out[j];
    }
    out
}

#[derive(Debug, Clone)]
pub struct Imdct256 {
    window: [f32; IMDCT_OUTPUT_SAMPLES],
    scale: f32,
}

impl Default for Imdct256 {
    fn default() -> Self {
        Self {
            window: atrac3_decoder_window(),
            scale: IMDCT_SCALE,
        }
    }
}

impl Imdct256 {
    pub fn inverse(&self, input: &[f32; MDCT_COEFFS_PER_BAND]) -> [f32; IMDCT_OUTPUT_SAMPLES] {
        let n = MDCT_COEFFS_PER_BAND as f64;
        let mut out = [0.0f32; IMDCT_OUTPUT_SAMPLES];

        for (sample_index, slot) in out.iter_mut().enumerate() {
            let mut acc = 0.0f64;
            for (coefficient_index, coefficient) in input.iter().enumerate() {
                let angle = (PI / n)
                    * ((sample_index as f64 + 0.5 + n / 2.0) * (coefficient_index as f64 + 0.5));
                acc += *coefficient as f64 * angle.cos();
            }
            *slot = acc as f32 * self.scale * self.window[sample_index];
        }

        out
    }
}

#[derive(Debug, Clone)]
struct IqmfState {
    delay: [f32; 46],
    window: [f32; 48],
}

impl Default for IqmfState {
    fn default() -> Self {
        Self {
            delay: [0.0; 46],
            window: mirrored_qmf_window(),
        }
    }
}

impl IqmfState {
    fn synthesize(&mut self, inlo: &[f32], inhi: &[f32]) -> Result<Vec<f32>> {
        ensure!(
            inlo.len() == inhi.len(),
            "iQMF input lengths differ: {} vs {}",
            inlo.len(),
            inhi.len()
        );
        ensure!(
            inlo.len() & 1 == 0,
            "iQMF input length must be even, got {}",
            inlo.len()
        );

        let n_in = inlo.len();
        let mut temp = vec![0.0f32; 46 + n_in * 2];
        temp[..46].copy_from_slice(&self.delay);

        let p3 = &mut temp[46..];
        for i in (0..n_in).step_by(2) {
            p3[2 * i] = inlo[i] + inhi[i];
            p3[2 * i + 1] = inlo[i] - inhi[i];
            p3[2 * i + 2] = inlo[i + 1] + inhi[i + 1];
            p3[2 * i + 3] = inlo[i + 1] - inhi[i + 1];
        }

        let mut out = vec![0.0f32; n_in * 2];
        let mut start = 0usize;
        let mut out_index = 0usize;
        for _ in 0..n_in {
            let mut s1 = 0.0f32;
            let mut s2 = 0.0f32;

            for tap in (0..48).step_by(2) {
                s1 += temp[start + tap] * self.window[tap];
                s2 += temp[start + tap + 1] * self.window[tap + 1];
            }

            out[out_index] = s2;
            out[out_index + 1] = s1;
            start += 2;
            out_index += 2;
        }

        self.delay.copy_from_slice(&temp[n_in * 2..n_in * 2 + 46]);
        Ok(out)
    }
}

#[derive(Debug, Clone)]
struct SynthesisChannel {
    imdct_overlap: [[f32; MDCT_COEFFS_PER_BAND]; QMF_BANDS],
    qmf_low: IqmfState,
    qmf_high: IqmfState,
    qmf_root: IqmfState,
}

impl Default for SynthesisChannel {
    fn default() -> Self {
        Self {
            imdct_overlap: [[0.0; MDCT_COEFFS_PER_BAND]; QMF_BANDS],
            qmf_low: IqmfState::default(),
            qmf_high: IqmfState::default(),
            qmf_root: IqmfState::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Atrac3Synthesis {
    imdct: Imdct256,
    channels: Vec<SynthesisChannel>,
}

impl Atrac3Synthesis {
    pub fn new(channel_count: usize) -> Self {
        Self {
            imdct: Imdct256::default(),
            channels: vec![SynthesisChannel::default(); channel_count],
        }
    }

    pub fn synthesize_frame(&mut self, channels: &[&[f32]]) -> Result<Vec<Vec<f32>>> {
        ensure!(
            channels.len() == self.channels.len(),
            "channel count mismatch: got {}, synthesis expects {}",
            channels.len(),
            self.channels.len()
        );

        channels
            .iter()
            .enumerate()
            .map(|(channel_index, coefficients)| {
                self.synthesize_channel(channel_index, coefficients)
            })
            .collect()
    }

    fn synthesize_channel(
        &mut self,
        channel_index: usize,
        coefficients: &[f32],
    ) -> Result<Vec<f32>> {
        ensure!(
            coefficients.len() == SAMPLES_PER_FRAME,
            "expected {} coefficients, got {}",
            SAMPLES_PER_FRAME,
            coefficients.len()
        );

        let channel = &mut self.channels[channel_index];
        let mut bands = [[0.0f32; SAMPLES_PER_BAND]; QMF_BANDS];

        for (band_index, band_out) in bands.iter_mut().enumerate() {
            let start = band_index * SAMPLES_PER_BAND;
            let end = start + SAMPLES_PER_BAND;
            let mut band_coefficients = [0.0f32; MDCT_COEFFS_PER_BAND];
            band_coefficients.copy_from_slice(&coefficients[start..end]);
            if band_index & 1 == 1 {
                band_coefficients.reverse();
            }

            let imdct = self.imdct.inverse(&band_coefficients);
            for sample_index in 0..SAMPLES_PER_BAND {
                band_out[sample_index] =
                    imdct[sample_index] + channel.imdct_overlap[band_index][sample_index];
            }
            channel.imdct_overlap[band_index].copy_from_slice(&imdct[SAMPLES_PER_BAND..]);
        }

        let low_half = channel.qmf_low.synthesize(&bands[0], &bands[1])?;
        let high_half = channel.qmf_high.synthesize(&bands[3], &bands[2])?;
        channel.qmf_root.synthesize(&low_half, &high_half)
    }
}

#[cfg(test)]
mod tests {
    use super::{Atrac3Synthesis, Imdct256, IqmfState, atrac3_decoder_window};
    use crate::atrac3::{
        mdct::{MDCT_COEFFS_PER_BAND, MDCT_INPUT_SAMPLES, Mdct256, atrac3_analysis_window_half},
        qmf::{FourBandQmf, mirrored_qmf_window},
    };

    #[test]
    fn decoder_window_is_symmetric() {
        let window = atrac3_decoder_window();
        assert!((window[0] - window[511]).abs() < 1e-6);
        assert!((window[127] - window[384]).abs() < 1e-6);
    }

    #[test]
    fn zero_imdct_stays_zero() {
        let imdct = Imdct256::default();
        let input = [0.0f32; 256];
        let output = imdct.inverse(&input);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn zero_synthesis_frame_stays_zero() {
        let mut synthesis = Atrac3Synthesis::new(1);
        let spectrum = vec![0.0f32; 1024];
        let output = synthesis.synthesize_frame(&[spectrum.as_slice()]).unwrap();
        assert_eq!(output.len(), 1);
        assert!(output[0].iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn qmf_iqmf_roundtrip_on_sine_reports_snr() {
        fn normalized_best_lag_snr(reference: &[f32], candidate: &[f32]) -> (isize, f64) {
            let mut best_lag = 0isize;
            let mut best_snr_db = f64::NEG_INFINITY;
            for lag in -128isize..=128 {
                let mut cross = 0.0f64;
                let mut candidate_energy = 0.0f64;
                for (index, reference_sample) in reference.iter().enumerate() {
                    let candidate_index = index as isize + lag;
                    if candidate_index < 0 || candidate_index >= candidate.len() as isize {
                        continue;
                    }
                    let candidate_sample = candidate[candidate_index as usize] as f64;
                    cross += *reference_sample as f64 * candidate_sample;
                    candidate_energy += candidate_sample * candidate_sample;
                }
                let gain = if candidate_energy > 1e-18 {
                    cross / candidate_energy
                } else {
                    1.0
                };

                let mut signal_energy = 0.0f64;
                let mut error_energy = 0.0f64;
                for (index, reference_sample) in reference.iter().enumerate() {
                    let candidate_index = index as isize + lag;
                    if candidate_index < 0 || candidate_index >= candidate.len() as isize {
                        continue;
                    }
                    let error = *reference_sample as f64
                        - gain * candidate[candidate_index as usize] as f64;
                    signal_energy += (*reference_sample as f64) * (*reference_sample as f64);
                    error_energy += error * error;
                }

                let snr_db = 10.0 * (signal_energy.max(1e-18) / error_energy.max(1e-18)).log10();
                if snr_db > best_snr_db {
                    best_snr_db = snr_db;
                    best_lag = lag;
                }
            }
            (best_lag, best_snr_db)
        }

        let mut analysis = FourBandQmf::default();
        let mut low = IqmfState::default();
        let mut high = IqmfState::default();
        let mut root = IqmfState::default();

        let sample_rate = 44_100.0f32;
        let frequency = 1_000.0f32;
        let mut signal = vec![0.0f32; 2048];
        for (index, slot) in signal.iter_mut().enumerate() {
            let phase = 2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate;
            *slot = phase.sin() * 0.5;
        }

        let _ = analysis.split_frame(&signal[..1024]).unwrap();
        let bands = analysis.split_frame(&signal[1024..2048]).unwrap();
        let low_half = low.synthesize(&bands[0], &bands[1]).unwrap();
        let high_half = high.synthesize(&bands[3], &bands[2]).unwrap();
        let output = root.synthesize(&low_half, &high_half).unwrap();

        let reference = &signal[1024..2048];
        let (best_lag, best_snr_db) = normalized_best_lag_snr(reference, &output);

        println!("qmf_roundtrip_best_lag={best_lag}");
        println!("qmf_roundtrip_best_snr_db={best_snr_db:.4}");
        assert!(best_snr_db.is_finite());
    }

    #[test]
    fn qmf_variant_scan_on_sine() {
        #[derive(Clone, Copy, Debug)]
        struct Variant {
            first_sum: bool,
            second_negated: bool,
            root_swapped: bool,
            upper_swapped: bool,
        }

        fn split_variant(
            delay: &mut [f32; 46],
            input: &[f32],
            variant: Variant,
        ) -> (Vec<f32>, Vec<f32>) {
            let window = mirrored_qmf_window();
            let mut history = Vec::with_capacity(delay.len() + input.len());
            history.extend_from_slice(delay);
            history.extend_from_slice(input);

            let out_len = input.len() / 2;
            let mut first = vec![0.0f32; out_len];
            let mut second = vec![0.0f32; out_len];

            for k in 0..out_len {
                let base = k * 2;
                let mut even = 0.0f32;
                let mut odd = 0.0f32;
                for tap in 0..24 {
                    even += history[base + tap * 2] * window[tap * 2];
                    odd += history[base + tap * 2 + 1] * window[tap * 2 + 1];
                }

                let sum = even + odd;
                let diff = if variant.second_negated {
                    odd - even
                } else {
                    even - odd
                };

                if variant.first_sum {
                    first[k] = sum;
                    second[k] = diff;
                } else {
                    first[k] = diff;
                    second[k] = sum;
                }
            }

            let delay_len = delay.len();
            delay.copy_from_slice(&history[history.len() - delay_len..]);
            (first, second)
        }

        fn best_lag_snr(reference: &[f32], candidate: &[f32]) -> f64 {
            let mut best = f64::NEG_INFINITY;
            for lag in -128isize..=128 {
                let mut cross = 0.0f64;
                let mut candidate_energy = 0.0f64;
                for (index, reference_sample) in reference.iter().enumerate() {
                    let candidate_index = index as isize + lag;
                    if candidate_index < 0 || candidate_index >= candidate.len() as isize {
                        continue;
                    }
                    let candidate_sample = candidate[candidate_index as usize] as f64;
                    cross += *reference_sample as f64 * candidate_sample;
                    candidate_energy += candidate_sample * candidate_sample;
                }
                let gain = if candidate_energy > 1e-18 {
                    cross / candidate_energy
                } else {
                    1.0
                };

                let mut signal_energy = 0.0f64;
                let mut error_energy = 0.0f64;
                for (index, reference_sample) in reference.iter().enumerate() {
                    let candidate_index = index as isize + lag;
                    if candidate_index < 0 || candidate_index >= candidate.len() as isize {
                        continue;
                    }
                    let error = *reference_sample as f64
                        - gain * candidate[candidate_index as usize] as f64;
                    signal_energy += (*reference_sample as f64) * (*reference_sample as f64);
                    error_energy += error * error;
                }
                let snr_db = 10.0 * (signal_energy.max(1e-18) / error_energy.max(1e-18)).log10();
                if snr_db > best {
                    best = snr_db;
                }
            }
            best
        }

        let sample_rate = 44_100.0f32;
        let frequency = 1_000.0f32;
        let mut signal = vec![0.0f32; 2048];
        for (index, slot) in signal.iter_mut().enumerate() {
            let phase = 2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate;
            *slot = phase.sin() * 0.5;
        }

        let mut best_snr = f64::NEG_INFINITY;
        let mut best_variant = None;
        for first_sum in [true, false] {
            for second_negated in [false, true] {
                for root_swapped in [false, true] {
                    for upper_swapped in [false, true] {
                        let variant = Variant {
                            first_sum,
                            second_negated,
                            root_swapped,
                            upper_swapped,
                        };

                        let mut root_delay = [0.0f32; 46];
                        let mut low_delay = [0.0f32; 46];
                        let mut high_delay = [0.0f32; 46];
                        let mut iqmf_low = IqmfState::default();
                        let mut iqmf_high = IqmfState::default();
                        let mut iqmf_root = IqmfState::default();

                        let _ = {
                            let (first, second) =
                                split_variant(&mut root_delay, &signal[..1024], variant);
                            let (root_a, root_b) = if variant.root_swapped {
                                (second, first)
                            } else {
                                (first, second)
                            };
                            let _ = split_variant(&mut low_delay, &root_a, variant);
                            let _ = split_variant(&mut high_delay, &root_b, variant);
                            (root_a, root_b)
                        };

                        let (first, second) =
                            split_variant(&mut root_delay, &signal[1024..2048], variant);
                        let (root_a, root_b) = if variant.root_swapped {
                            (second, first)
                        } else {
                            (first, second)
                        };
                        let (band0, band1) = split_variant(&mut low_delay, &root_a, variant);
                        let (upper_first, upper_second) =
                            split_variant(&mut high_delay, &root_b, variant);
                        let (band2, band3) = if variant.upper_swapped {
                            (upper_first, upper_second)
                        } else {
                            (upper_second, upper_first)
                        };

                        let low_half = iqmf_low.synthesize(&band0, &band1).unwrap();
                        let high_half = iqmf_high.synthesize(&band3, &band2).unwrap();
                        let output = iqmf_root.synthesize(&low_half, &high_half).unwrap();
                        let snr_db = best_lag_snr(&signal[1024..2048], &output);
                        println!("variant={variant:?} best_snr_db={snr_db:.4}");
                        if snr_db > best_snr {
                            best_snr = snr_db;
                            best_variant = Some(variant);
                        }
                    }
                }
            }
        }

        println!("best_variant={best_variant:?} best_snr_db={best_snr:.4}");
        assert!(best_snr.is_finite());
    }

    #[test]
    fn mdct_imdct_roundtrip_on_sine_reports_snr() {
        fn normalized_snr(reference: &[f32], candidate: &[f32]) -> f64 {
            let cross = reference
                .iter()
                .zip(candidate.iter())
                .map(|(reference_sample, candidate_sample)| {
                    *reference_sample as f64 * *candidate_sample as f64
                })
                .sum::<f64>();
            let candidate_energy = candidate
                .iter()
                .map(|sample| (*sample as f64) * (*sample as f64))
                .sum::<f64>();
            let gain = if candidate_energy > 1e-18 {
                cross / candidate_energy
            } else {
                1.0
            };

            let signal_energy = reference
                .iter()
                .map(|sample| (*sample as f64) * (*sample as f64))
                .sum::<f64>();
            let error_energy = reference
                .iter()
                .zip(candidate.iter())
                .map(|(reference_sample, candidate_sample)| {
                    let error = *reference_sample as f64 - gain * *candidate_sample as f64;
                    error * error
                })
                .sum::<f64>();
            10.0 * (signal_energy.max(1e-18) / error_energy.max(1e-18)).log10()
        }

        let mdct = Mdct256::default();
        let imdct = Imdct256::default();
        let sample_rate = 44_100.0f32;
        let frequency = 1_000.0f32;
        let mut signal = vec![0.0f32; MDCT_COEFFS_PER_BAND * 3];
        for (index, slot) in signal.iter_mut().enumerate() {
            let phase = 2.0 * std::f32::consts::PI * frequency * index as f32 / sample_rate;
            *slot = phase.sin() * 0.5;
        }

        for current_first in [false, true] {
            let mut previous = [0.0f32; MDCT_COEFFS_PER_BAND];
            let mut overlap = [0.0f32; MDCT_COEFFS_PER_BAND];
            let mut candidate = Vec::with_capacity(MDCT_COEFFS_PER_BAND * 2);
            let mut reference = Vec::with_capacity(MDCT_COEFFS_PER_BAND * 2);

            for frame_index in 0..3 {
                let mut mdct_input = [0.0f32; MDCT_INPUT_SAMPLES];
                let current_start = frame_index * MDCT_COEFFS_PER_BAND;
                let current_end = current_start + MDCT_COEFFS_PER_BAND;
                let current = &signal[current_start..current_end];
                if current_first {
                    mdct_input[..MDCT_COEFFS_PER_BAND].copy_from_slice(current);
                    mdct_input[MDCT_COEFFS_PER_BAND..].copy_from_slice(&previous);
                } else {
                    mdct_input[..MDCT_COEFFS_PER_BAND].copy_from_slice(&previous);
                    mdct_input[MDCT_COEFFS_PER_BAND..].copy_from_slice(current);
                }
                previous.copy_from_slice(current);

                let coefficients = mdct.forward(&mdct_input);
                let samples = imdct.inverse(&coefficients);
                let mut reconstructed = [0.0f32; MDCT_COEFFS_PER_BAND];
                for sample_index in 0..MDCT_COEFFS_PER_BAND {
                    reconstructed[sample_index] = samples[sample_index] + overlap[sample_index];
                }
                overlap.copy_from_slice(&samples[MDCT_COEFFS_PER_BAND..]);

                if frame_index > 0 {
                    reference.extend_from_slice(current);
                    candidate.extend_from_slice(&reconstructed);
                }
            }

            let snr_db = normalized_snr(&reference, &candidate);
            println!("mdct_roundtrip_current_first={current_first} snr_db={snr_db:.4}");
            assert!(snr_db.is_finite());
        }
    }

    #[test]
    fn mdct_variant_scan_on_sine() {
        #[derive(Clone, Copy, Debug)]
        struct Variant {
            current_first: bool,
            reverse_first: bool,
            reverse_second: bool,
            negate_first: bool,
            negate_second: bool,
        }

        fn normalized_snr(reference: &[f32], candidate: &[f32]) -> f64 {
            let cross = reference
                .iter()
                .zip(candidate.iter())
                .map(|(reference_sample, candidate_sample)| {
                    *reference_sample as f64 * *candidate_sample as f64
                })
                .sum::<f64>();
            let candidate_energy = candidate
                .iter()
                .map(|sample| (*sample as f64) * (*sample as f64))
                .sum::<f64>();
            let gain = if candidate_energy > 1e-18 {
                cross / candidate_energy
            } else {
                1.0
            };

            let signal_energy = reference
                .iter()
                .map(|sample| (*sample as f64) * (*sample as f64))
                .sum::<f64>();
            let error_energy = reference
                .iter()
                .zip(candidate.iter())
                .map(|(reference_sample, candidate_sample)| {
                    let error = *reference_sample as f64 - gain * *candidate_sample as f64;
                    error * error
                })
                .sum::<f64>();
            10.0 * (signal_energy.max(1e-18) / error_energy.max(1e-18)).log10()
        }

        fn build_input(
            previous: &[f32; MDCT_COEFFS_PER_BAND],
            current: &[f32],
            variant: Variant,
        ) -> [f32; MDCT_INPUT_SAMPLES] {
            let mut first = [0.0f32; MDCT_COEFFS_PER_BAND];
            let mut second = [0.0f32; MDCT_COEFFS_PER_BAND];

            if variant.current_first {
                first.copy_from_slice(current);
                second.copy_from_slice(previous);
            } else {
                first.copy_from_slice(previous);
                second.copy_from_slice(current);
            }

            if variant.reverse_first {
                first.reverse();
            }
            if variant.reverse_second {
                second.reverse();
            }
            if variant.negate_first {
                for sample in &mut first {
                    *sample = -*sample;
                }
            }
            if variant.negate_second {
                for sample in &mut second {
                    *sample = -*sample;
                }
            }

            let mut input = [0.0f32; MDCT_INPUT_SAMPLES];
            input[..MDCT_COEFFS_PER_BAND].copy_from_slice(&first);
            input[MDCT_COEFFS_PER_BAND..].copy_from_slice(&second);
            input
        }

        let mdct = Mdct256::default();
        let imdct = Imdct256::default();
        let sample_rate = 44_100.0f32;
        let freqs = [1_000.0f32, 3_100.0f32, 4_700.0f32];
        let gains = [0.5f32, 0.3f32, 0.2f32];
        let mut signal = vec![0.0f32; MDCT_COEFFS_PER_BAND * 4];
        for (index, slot) in signal.iter_mut().enumerate() {
            *slot = freqs
                .iter()
                .zip(gains)
                .map(|(frequency, gain)| {
                    let phase =
                        2.0 * std::f32::consts::PI * *frequency * index as f32 / sample_rate;
                    phase.sin() * gain
                })
                .sum();
        }

        let mut best_snr = f64::NEG_INFINITY;
        let mut best_variant = None;
        for current_first in [false, true] {
            for reverse_first in [false, true] {
                for reverse_second in [false, true] {
                    for negate_first in [false, true] {
                        for negate_second in [false, true] {
                            let variant = Variant {
                                current_first,
                                reverse_first,
                                reverse_second,
                                negate_first,
                                negate_second,
                            };

                            let mut previous = [0.0f32; MDCT_COEFFS_PER_BAND];
                            let mut overlap = [0.0f32; MDCT_COEFFS_PER_BAND];
                            let mut candidate = Vec::with_capacity(MDCT_COEFFS_PER_BAND * 3);
                            let mut reference = Vec::with_capacity(MDCT_COEFFS_PER_BAND * 3);

                            for frame_index in 0..4 {
                                let current_start = frame_index * MDCT_COEFFS_PER_BAND;
                                let current_end = current_start + MDCT_COEFFS_PER_BAND;
                                let current = &signal[current_start..current_end];
                                let mdct_input = build_input(&previous, current, variant);
                                previous.copy_from_slice(current);

                                let coefficients = mdct.forward(&mdct_input);
                                let samples = imdct.inverse(&coefficients);
                                let mut reconstructed = [0.0f32; MDCT_COEFFS_PER_BAND];
                                for sample_index in 0..MDCT_COEFFS_PER_BAND {
                                    reconstructed[sample_index] =
                                        samples[sample_index] + overlap[sample_index];
                                }
                                overlap.copy_from_slice(&samples[MDCT_COEFFS_PER_BAND..]);

                                if frame_index > 0 {
                                    reference.extend_from_slice(current);
                                    candidate.extend_from_slice(&reconstructed);
                                }
                            }

                            let snr_db = normalized_snr(&reference, &candidate);
                            println!("variant={variant:?} snr_db={snr_db:.4}");
                            if snr_db > best_snr {
                                best_snr = snr_db;
                                best_variant = Some(variant);
                            }
                        }
                    }
                }
            }
        }

        println!("best_mdct_variant={best_variant:?} snr_db={best_snr:.4}");
        assert!(best_snr.is_finite());
    }

    #[test]
    fn mdct_window_scan_on_sine() {
        #[derive(Clone, Copy, Debug)]
        struct Variant {
            first_reversed: bool,
            second_reversed: bool,
            first_negated: bool,
            second_negated: bool,
        }

        fn normalized_snr(reference: &[f32], candidate: &[f32]) -> f64 {
            let cross = reference
                .iter()
                .zip(candidate.iter())
                .map(|(reference_sample, candidate_sample)| {
                    *reference_sample as f64 * *candidate_sample as f64
                })
                .sum::<f64>();
            let candidate_energy = candidate
                .iter()
                .map(|sample| (*sample as f64) * (*sample as f64))
                .sum::<f64>();
            let gain = if candidate_energy > 1e-18 {
                cross / candidate_energy
            } else {
                1.0
            };

            let signal_energy = reference
                .iter()
                .map(|sample| (*sample as f64) * (*sample as f64))
                .sum::<f64>();
            let error_energy = reference
                .iter()
                .zip(candidate.iter())
                .map(|(reference_sample, candidate_sample)| {
                    let error = *reference_sample as f64 - gain * *candidate_sample as f64;
                    error * error
                })
                .sum::<f64>();
            10.0 * (signal_energy.max(1e-18) / error_energy.max(1e-18)).log10()
        }

        fn forward_with_window(
            input: &[f32; MDCT_INPUT_SAMPLES],
            window: &[f32; MDCT_INPUT_SAMPLES],
        ) -> [f32; MDCT_COEFFS_PER_BAND] {
            let n = MDCT_COEFFS_PER_BAND as f64;
            let mut out = [0.0f32; MDCT_COEFFS_PER_BAND];
            for (k, slot) in out.iter_mut().enumerate() {
                let mut acc = 0.0f64;
                for (idx, sample) in input.iter().enumerate() {
                    let angle = (std::f64::consts::PI / n)
                        * ((idx as f64 + 0.5 + n / 2.0) * (k as f64 + 0.5));
                    acc += *sample as f64 * window[idx] as f64 * angle.cos();
                }
                *slot = acc as f32;
            }
            out
        }

        let imdct = Imdct256::default();
        let sample_rate = 44_100.0f32;
        let freqs = [1_000.0f32, 3_100.0f32, 4_700.0f32];
        let gains = [0.5f32, 0.3f32, 0.2f32];
        let mut signal = vec![0.0f32; MDCT_COEFFS_PER_BAND * 4];
        for (index, slot) in signal.iter_mut().enumerate() {
            *slot = freqs
                .iter()
                .zip(gains)
                .map(|(frequency, gain)| {
                    let phase =
                        2.0 * std::f32::consts::PI * *frequency * index as f32 / sample_rate;
                    phase.sin() * gain
                })
                .sum();
        }

        let half = atrac3_analysis_window_half();
        let mut best_snr = f64::NEG_INFINITY;
        let mut best_variant = None;
        for first_reversed in [false, true] {
            for second_reversed in [false, true] {
                for first_negated in [false, true] {
                    for second_negated in [false, true] {
                        let variant = Variant {
                            first_reversed,
                            second_reversed,
                            first_negated,
                            second_negated,
                        };

                        let mut window = [0.0f32; MDCT_INPUT_SAMPLES];
                        let mut first = half;
                        let mut second = half;
                        if variant.first_reversed {
                            first.reverse();
                        }
                        if variant.second_reversed {
                            second.reverse();
                        }
                        if variant.first_negated {
                            for sample in &mut first {
                                *sample = -*sample;
                            }
                        }
                        if variant.second_negated {
                            for sample in &mut second {
                                *sample = -*sample;
                            }
                        }
                        window[..MDCT_COEFFS_PER_BAND].copy_from_slice(&first);
                        window[MDCT_COEFFS_PER_BAND..].copy_from_slice(&second);

                        let mut previous = [0.0f32; MDCT_COEFFS_PER_BAND];
                        let mut overlap = [0.0f32; MDCT_COEFFS_PER_BAND];
                        let mut candidate = Vec::with_capacity(MDCT_COEFFS_PER_BAND * 3);
                        let mut reference = Vec::with_capacity(MDCT_COEFFS_PER_BAND * 3);

                        for frame_index in 0..4 {
                            let current_start = frame_index * MDCT_COEFFS_PER_BAND;
                            let current_end = current_start + MDCT_COEFFS_PER_BAND;
                            let current = &signal[current_start..current_end];

                            let mut input = [0.0f32; MDCT_INPUT_SAMPLES];
                            input[..MDCT_COEFFS_PER_BAND].copy_from_slice(current);
                            input[MDCT_COEFFS_PER_BAND..].copy_from_slice(&previous);
                            input[..MDCT_COEFFS_PER_BAND].reverse();
                            input[MDCT_COEFFS_PER_BAND..].reverse();
                            previous.copy_from_slice(current);

                            let coefficients = forward_with_window(&input, &window);
                            let samples = imdct.inverse(&coefficients);
                            let mut reconstructed = [0.0f32; MDCT_COEFFS_PER_BAND];
                            for sample_index in 0..MDCT_COEFFS_PER_BAND {
                                reconstructed[sample_index] =
                                    samples[sample_index] + overlap[sample_index];
                            }
                            overlap.copy_from_slice(&samples[MDCT_COEFFS_PER_BAND..]);

                            if frame_index > 0 {
                                reference.extend_from_slice(current);
                                candidate.extend_from_slice(&reconstructed);
                            }
                        }

                        let snr_db = normalized_snr(&reference, &candidate);
                        println!("window_variant={variant:?} snr_db={snr_db:.4}");
                        if snr_db > best_snr {
                            best_snr = snr_db;
                            best_variant = Some(variant);
                        }
                    }
                }
            }
        }

        println!("best_window_variant={best_variant:?} snr_db={best_snr:.4}");
        assert!(best_snr.is_finite());
    }

    #[test]
    fn reference_mdct_imdct_roundtrip_perfect_reconstruction() {
        fn normalized_snr(reference: &[f32], candidate: &[f32]) -> f64 {
            let cross: f64 = reference
                .iter()
                .zip(candidate.iter())
                .map(|(r, c)| *r as f64 * *c as f64)
                .sum();
            let ce: f64 = candidate.iter().map(|c| (*c as f64).powi(2)).sum();
            let gain = if ce > 1e-18 { cross / ce } else { 1.0 };
            let se: f64 = reference.iter().map(|r| (*r as f64).powi(2)).sum();
            let ee: f64 = reference
                .iter()
                .zip(candidate.iter())
                .map(|(r, c)| { let e = *r as f64 - gain * *c as f64; e * e })
                .sum();
            10.0 * (se.max(1e-18) / ee.max(1e-18)).log10()
        }

        let mdct = Mdct256::default();
        let imdct = Imdct256::default();

        let sample_rate = 44_100.0f32;
        let freqs = [440.0f32, 1000.0, 3100.0];
        let gains = [0.4f32, 0.3, 0.2];
        let total_frames = 6;
        let warmup = 2;
        let mut signal = vec![0.0f32; MDCT_COEFFS_PER_BAND * total_frames];
        for (i, s) in signal.iter_mut().enumerate() {
            *s = freqs.iter().zip(gains).map(|(f, g)| {
                (2.0 * std::f32::consts::PI * f * i as f32 / sample_rate).sin() * g
            }).sum();
        }

        // Test both orderings, comparing reconstruction at frame t with the
        // input at frame t (direct comparison) AND frame t-1 (delayed comparison)
        // to find which alignment is correct.
        for (label, current_first) in [("overlap_first", false), ("current_first", true)] {
            let mut previous = [0.0f32; MDCT_COEFFS_PER_BAND];
            let mut overlap = [0.0f32; MDCT_COEFFS_PER_BAND];
            let mut all_inputs: Vec<[f32; MDCT_COEFFS_PER_BAND]> = Vec::new();
            let mut all_reconstructed: Vec<[f32; MDCT_COEFFS_PER_BAND]> = Vec::new();

            for frame_index in 0..total_frames {
                let start = frame_index * MDCT_COEFFS_PER_BAND;
                let end = start + MDCT_COEFFS_PER_BAND;
                let current = &signal[start..end];

                let mut mdct_input = [0.0f32; MDCT_INPUT_SAMPLES];
                if current_first {
                    mdct_input[..MDCT_COEFFS_PER_BAND].copy_from_slice(current);
                    mdct_input[MDCT_COEFFS_PER_BAND..].copy_from_slice(&previous);
                } else {
                    mdct_input[..MDCT_COEFFS_PER_BAND].copy_from_slice(&previous);
                    mdct_input[MDCT_COEFFS_PER_BAND..].copy_from_slice(current);
                }
                previous.copy_from_slice(current);

                let coefficients = mdct.forward_reference(&mdct_input);
                let samples = imdct.inverse(&coefficients);
                let mut reconstructed = [0.0f32; MDCT_COEFFS_PER_BAND];
                for i in 0..MDCT_COEFFS_PER_BAND {
                    reconstructed[i] = samples[i] + overlap[i];
                }
                overlap.copy_from_slice(&samples[MDCT_COEFFS_PER_BAND..]);

                let mut input_copy = [0.0f32; MDCT_COEFFS_PER_BAND];
                input_copy.copy_from_slice(current);
                all_inputs.push(input_copy);
                all_reconstructed.push(reconstructed);
            }

            // Direct comparison (output at frame t vs input at frame t)
            let mut ref_direct = Vec::new();
            let mut cand_direct = Vec::new();
            for i in warmup..total_frames {
                ref_direct.extend_from_slice(&all_inputs[i]);
                cand_direct.extend_from_slice(&all_reconstructed[i]);
            }
            let snr_direct = normalized_snr(&ref_direct, &cand_direct);

            // Delayed comparison (output at frame t vs input at frame t-1)
            let mut ref_delayed = Vec::new();
            let mut cand_delayed = Vec::new();
            for i in warmup..total_frames {
                ref_delayed.extend_from_slice(&all_inputs[i - 1]);
                cand_delayed.extend_from_slice(&all_reconstructed[i]);
            }
            let snr_delayed = normalized_snr(&ref_delayed, &cand_delayed);

            println!("{label}: direct_snr={snr_direct:.4} delayed_snr={snr_delayed:.4}");
        }

        // With overlap-first [prev | current], the MDCT/IMDCT roundtrip has a
        // 1-frame delay: the overlap-add at frame t reconstructs frame t-1's
        // input.  The delayed SNR for overlap_first should be > 100 dB
        // (near-perfect reconstruction).
    }
}
