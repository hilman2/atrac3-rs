use std::f64::consts::PI;

use anyhow::{Result, ensure};

use super::{
    SAMPLES_PER_BAND,
    sound_unit::{GainBand, GainPoint},
};

pub const GAIN_HISTORY_SLOTS: usize = 32;
pub const GAIN_CURVE_SLOTS: usize = 64;
pub const GAIN_CURVE_SAMPLES: usize = 256;
pub const WINDOW_EDGE_ZERO_SAMPLES: usize = 32;

pub const UNITY_GAIN_LEVEL_CODE: u8 = 4;
pub const GAIN_LEVEL_CODE_COUNT: usize = 16;
const GAIN_TRIGGER_RATIO: f32 = 1.85;
const GAIN_MIN_ABS_LEVEL: f32 = 1e-4;
const GAIN_LEVEL_EXPONENTS: [i32; GAIN_LEVEL_CODE_COUNT] =
    [-6, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
const GAIN_INTERPOLATION_STEPS: [f32; 12] = [
    0.594_604_5,
    0.707_092_3,
    0.840_881_35,
    0.353_546_14,
    0.5,
    0.707_092_3,
    0.210_235_6,
    0.353_546_14,
    0.594_604_5,
    0.125,
    0.25,
    0.5,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecoderWindowKind {
    Full,
    ZeroTail,
    ZeroHead,
    ZeroEdges,
}

#[derive(Debug, Clone)]
pub struct GainCurve {
    pub samples: [f32; GAIN_CURVE_SAMPLES],
    pub first_change_sample: usize,
}

pub fn combined_gain_history(
    previous: &[f32; GAIN_HISTORY_SLOTS],
    current: &[f32; GAIN_HISTORY_SLOTS],
) -> [f32; GAIN_CURVE_SLOTS] {
    let mut out = [0.0f32; GAIN_CURVE_SLOTS];
    out[..GAIN_HISTORY_SLOTS].copy_from_slice(previous);
    out[GAIN_HISTORY_SLOTS..].copy_from_slice(current);
    out
}

pub fn combined_gain_profile(
    current: &[f32; GAIN_HISTORY_SLOTS],
    previous: &[f32; GAIN_HISTORY_SLOTS],
) -> [f32; GAIN_CURVE_SLOTS] {
    let mut out = [0.0f32; GAIN_CURVE_SLOTS];
    out[..GAIN_HISTORY_SLOTS].copy_from_slice(current);
    out[GAIN_HISTORY_SLOTS..].copy_from_slice(previous);
    out
}

pub fn estimate_envelope_slots(samples: &[f32]) -> Result<[f32; GAIN_HISTORY_SLOTS]> {
    ensure!(
        samples.len() == SAMPLES_PER_BAND,
        "gain envelope expects {} band samples, got {}",
        SAMPLES_PER_BAND,
        samples.len()
    );

    let mut out = [0.0f32; GAIN_HISTORY_SLOTS];
    for (slot_index, chunk) in samples
        .chunks_exact(SAMPLES_PER_BAND / GAIN_HISTORY_SLOTS)
        .enumerate()
    {
        out[slot_index] = chunk
            .iter()
            .map(|sample| sample.abs())
            .fold(0.0f32, f32::max);
    }
    Ok(out)
}

pub fn decoder_window_kind(flag_a_active: bool, flag_b_active: bool) -> DecoderWindowKind {
    match (flag_a_active, flag_b_active) {
        (false, false) => DecoderWindowKind::Full,
        (false, true) => DecoderWindowKind::ZeroTail,
        (true, false) => DecoderWindowKind::ZeroHead,
        (true, true) => DecoderWindowKind::ZeroEdges,
    }
}

pub fn decoder_window_table(kind: DecoderWindowKind) -> [f32; GAIN_CURVE_SAMPLES] {
    let mut out = [0.0f32; GAIN_CURVE_SAMPLES];
    for (index, sample) in out.iter_mut().enumerate() {
        *sample = (((index as f64 + 0.5) * PI) / GAIN_CURVE_SAMPLES as f64).sin() as f32;
    }

    match kind {
        DecoderWindowKind::Full => {}
        DecoderWindowKind::ZeroTail => {
            out[GAIN_CURVE_SAMPLES - WINDOW_EDGE_ZERO_SAMPLES..].fill(0.0);
        }
        DecoderWindowKind::ZeroHead => {
            out[..WINDOW_EDGE_ZERO_SAMPLES].fill(0.0);
        }
        DecoderWindowKind::ZeroEdges => {
            out[..WINDOW_EDGE_ZERO_SAMPLES].fill(0.0);
            out[GAIN_CURVE_SAMPLES - WINDOW_EDGE_ZERO_SAMPLES..].fill(0.0);
        }
    }

    out
}

pub fn build_gain_curve(current: &GainBand, previous: &GainBand) -> Result<GainCurve> {
    validate_gain_band(previous)?;
    validate_gain_band(current)?;
    let mut slot_exponents = [0i32; GAIN_CURVE_SLOTS];

    let mut previous_slot = 0usize;
    for point in &previous.points {
        let exponent = GAIN_LEVEL_EXPONENTS[point.level as usize];
        let end_slot = (point.location as usize + GAIN_HISTORY_SLOTS).min(GAIN_CURVE_SLOTS - 1);
        while previous_slot <= end_slot {
            slot_exponents[previous_slot] = exponent;
            previous_slot += 1;
        }
        if previous_slot == GAIN_CURVE_SLOTS {
            break;
        }
    }

    let mut current_slot = 0usize;
    for point in &current.points {
        let exponent = GAIN_LEVEL_EXPONENTS[point.level as usize];
        let end_slot = (point.location as usize).min(GAIN_CURVE_SLOTS - 1);
        while current_slot <= end_slot {
            slot_exponents[current_slot] += exponent;
            current_slot += 1;
        }
        if current_slot == GAIN_CURVE_SLOTS {
            break;
        }
    }

    let mut samples = [0.0f32; GAIN_CURVE_SAMPLES];
    let mut previous_exponent = slot_exponents[GAIN_CURVE_SLOTS - 1];
    let mut current_gain = gain_exponent_to_scale(previous_exponent);

    for slot_index in (0..GAIN_CURVE_SLOTS).rev() {
        let exponent = slot_exponents[slot_index];
        let base = slot_index * 4;
        if exponent == previous_exponent {
            samples[base] = current_gain;
            samples[base + 1] = current_gain;
            samples[base + 2] = current_gain;
            samples[base + 3] = current_gain;
        } else if exponent < previous_exponent {
            let old_gain = gain_exponent_to_scale(previous_exponent);
            let interp_index = ((previous_exponent - exponent - 1) as usize * 3)
                .min(GAIN_INTERPOLATION_STEPS.len() - 3);
            samples[base + 1] = old_gain * GAIN_INTERPOLATION_STEPS[interp_index];
            samples[base + 2] = old_gain * GAIN_INTERPOLATION_STEPS[interp_index + 1];
            samples[base + 3] = old_gain * GAIN_INTERPOLATION_STEPS[interp_index + 2];
            current_gain = gain_exponent_to_scale(exponent);
            samples[base] = current_gain;
        } else {
            current_gain = gain_exponent_to_scale(exponent);
            let interp_index = ((exponent - previous_exponent - 1) as usize * 3)
                .min(GAIN_INTERPOLATION_STEPS.len() - 3);
            samples[base + 1] = current_gain * GAIN_INTERPOLATION_STEPS[interp_index + 2];
            samples[base + 2] = current_gain * GAIN_INTERPOLATION_STEPS[interp_index + 1];
            samples[base + 3] = current_gain * GAIN_INTERPOLATION_STEPS[interp_index];
            samples[base] = current_gain;
        }
        previous_exponent = exponent;
    }

    let first_change_sample = first_non_unity_sample(&samples);

    Ok(GainCurve {
        samples,
        first_change_sample: first_change_sample.min(GAIN_CURVE_SAMPLES - 1),
    })
}

pub fn estimate_gain_band(
    current: &[f32; GAIN_HISTORY_SLOTS],
    previous: &[f32; GAIN_HISTORY_SLOTS],
    band_index: usize,
    history_peak_state: f32,
) -> GainBand {
    let history = combined_gain_profile(current, previous);
    let mode = band_index.min(7) as i32;
    let coarse = coarse_history_maxima(current);

    let mut scan_max = coarse[7];
    let limit = band_history_slots(band_index);
    for &value in &history[GAIN_HISTORY_SLOTS..limit] {
        scan_max = scan_max.max(value);
    }
    scan_max = scan_max.max(GAIN_MIN_ABS_LEVEL);

    let threshold_mul = gain_threshold_multiplier(mode);
    let mut threshold = scan_max * threshold_mul;
    let mut remaining_steps = 4i32;
    let mut positions = [32i32; 8];
    let mut level_deltas = [0i32; 8];
    let mut coarse_insert = 7usize;

    for coarse_index in (0..coarse.len()).rev() {
        let peak = coarse[coarse_index];
        if scan_max <= peak {
            if peak > GAIN_MIN_ABS_LEVEL && peak > threshold {
                coarse_insert -= 1;
                positions[coarse_insert] = coarse_location(&history, coarse_index, threshold) as i32;
                let step = gain_step_from_ratio(peak, scan_max).min(remaining_steps);
                remaining_steps -= step;
                level_deltas[coarse_insert] = -step;

                if remaining_steps < 1 || coarse_insert == 5 {
                    break;
                }
            }

            threshold = peak * threshold_mul;
            scan_max = peak;
        }
    }

    let backward_start = coarse_insert;
    let mut forward_count = 0usize;
    let mut additional_steps = 15 - remaining_steps;
    if additional_steps > 0 {
        let mut running_peak = history_peak_state
            .max(history[0])
            .max(GAIN_MIN_ABS_LEVEL);
        let mut running_threshold = running_peak * forward_threshold_multiplier(mode);
        let scan_limit = positions[backward_start].clamp(0, 32) as usize;

        for slot in 0..scan_limit {
            let value = history[slot + 1];
            if value < running_peak {
                continue;
            }
            if value <= GAIN_MIN_ABS_LEVEL || value <= running_threshold {
                running_threshold = value * forward_threshold_multiplier(mode);
                running_peak = value;
                continue;
            }

            positions[forward_count] = slot as i32;
            let mut step = gain_step_from_ratio(value, running_peak);
            if forward_count > 0
                && positions[forward_count - 1] == slot as i32 - 1
                && level_deltas[forward_count - 1] <= step
            {
                forward_count -= 1;
                additional_steps += level_deltas[forward_count];
                step += level_deltas[forward_count];
            }
            if step > additional_steps {
                step = additional_steps;
            }
            additional_steps -= step;
            level_deltas[forward_count] = step;
            forward_count += 1;

            if forward_count == backward_start || additional_steps < 1 {
                break;
            }

            running_threshold = value * forward_threshold_multiplier(mode);
            running_peak = value;
        }
    }

    let total_points = merge_backward_points(
        &mut positions,
        &mut level_deltas,
        forward_count,
        backward_start,
    );
    if total_points == 0 {
        return GainBand::default();
    }

    let mut running_level = UNITY_GAIN_LEVEL_CODE as i32;
    for index in (0..total_points).rev() {
        running_level += level_deltas[index];
        level_deltas[index] = running_level;
    }

    let mut points = Vec::with_capacity(total_points);
    for index in 0..total_points {
        let level = level_deltas[index].clamp(0, (GAIN_LEVEL_CODE_COUNT - 1) as i32) as u8;
        let location = positions[index].clamp(0, 31) as u8;
        points.push(GainPoint { level, location });
    }

    GainBand { points }
}

fn band_history_slots(band_index: usize) -> usize {
    (8usize.saturating_sub(band_index.min(7))) * 8
}

fn coarse_history_maxima(previous: &[f32; GAIN_HISTORY_SLOTS]) -> [f32; 8] {
    let mut coarse = [0.0f32; 8];
    for (group, slot_group) in previous.chunks_exact(4).enumerate() {
        coarse[group] = slot_group.iter().copied().fold(0.0f32, f32::max);
    }
    coarse
}

fn gain_threshold_multiplier(mode: i32) -> f32 {
    if mode == -1 { 2.0 } else { GAIN_TRIGGER_RATIO }
}

fn forward_threshold_multiplier(mode: i32) -> f32 {
    if mode == -1 { 2.0 } else { 1.6 }
}

fn coarse_location(
    history: &[f32; GAIN_CURVE_SLOTS],
    coarse_index: usize,
    threshold: f32,
) -> usize {
    let base = coarse_index * 4;
    let mut location = base;
    if coarse_index != 0
        && history.get(base + 4).copied().unwrap_or(0.0) < threshold
        && history[base + 3] < threshold
    {
        location = base + 3;
        if history[base + 2] < threshold {
            location = base + 2;
        }
    }
    location
}

fn gain_step_from_ratio(peak: f32, baseline: f32) -> i32 {
    let ratio = (peak / baseline.max(GAIN_MIN_ABS_LEVEL)) * std::f32::consts::SQRT_2;
    let bits = ratio.max(1.0).to_bits();
    ((bits >> 23) as i32 - 127).max(0)
}

fn merge_backward_points(
    positions: &mut [i32; 8],
    level_deltas: &mut [i32; 8],
    forward_count: usize,
    backward_start: usize,
) -> usize {
    let mut write_index = forward_count;
    let mut read_index = backward_start;
    while read_index < 7 {
        positions[write_index] = positions[read_index];
        level_deltas[write_index] = level_deltas[read_index];
        write_index += 1;
        read_index += 1;
    }
    write_index
}

fn gain_exponent_to_scale(exponent: i32) -> f32 {
    if exponent < 0 {
        1.0 / (1u32 << (-exponent as u32)) as f32
    } else {
        (1u32 << exponent as u32) as f32
    }
}

fn first_non_unity_sample(samples: &[f32; GAIN_CURVE_SAMPLES]) -> usize {
    samples
        .iter()
        .position(|sample| (*sample - 1.0).abs() > 1e-6)
        .unwrap_or(GAIN_CURVE_SAMPLES - 1)
}

fn validate_gain_band(band: &GainBand) -> Result<()> {
    let mut previous_location = None;
    for point in &band.points {
        ensure!(
            (point.level as usize) < GAIN_LEVEL_CODE_COUNT,
            "gain level code {} exceeds table size {}",
            point.level,
            GAIN_LEVEL_CODE_COUNT
        );
        if let Some(last_location) = previous_location {
            ensure!(
                point.location > last_location,
                "gain locations must be strictly increasing: {} then {}",
                last_location,
                point.location
            );
        }
        previous_location = Some(point.location);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DecoderWindowKind, GAIN_CURVE_SAMPLES, GAIN_HISTORY_SLOTS, WINDOW_EDGE_ZERO_SAMPLES,
        build_gain_curve, combined_gain_history, decoder_window_table, estimate_envelope_slots,
        estimate_gain_band,
    };
    use crate::atrac3::sound_unit::{GainBand, GainPoint};

    #[test]
    fn empty_gain_bands_produce_unity_curve() {
        let curve = build_gain_curve(&GainBand::default(), &GainBand::default()).unwrap();
        assert!(
            curve
                .samples
                .iter()
                .all(|sample| (*sample - 1.0).abs() < 1e-7)
        );
        assert_eq!(curve.first_change_sample, GAIN_CURVE_SAMPLES - 1);
    }

    #[test]
    fn gain_transition_changes_curve_prefix() {
        let current = GainBand {
            points: vec![GainPoint {
                level: 7,
                location: 0,
            }],
        };
        let curve = build_gain_curve(&current, &GainBand::default()).unwrap();
        assert!(curve.samples[0] > 1.0);
        assert!(curve.samples[1] > 1.0);
        assert!(curve.samples[3] > 1.0);
        assert!((curve.samples[4] - 1.0).abs() < 1e-6);
        assert_eq!(curve.first_change_sample, 0);
    }

    #[test]
    fn previous_gain_point_interpolates_over_eight_samples() {
        let previous = GainBand {
            points: vec![GainPoint {
                level: 8,
                location: 1,
            }],
        };
        let curve = build_gain_curve(&GainBand::default(), &previous).unwrap();
        assert!((curve.samples[0] - 4.0).abs() < 1e-6);
        assert!((curve.samples[131] - 4.0).abs() < 1e-6);
        assert!(curve.samples[132] > 1.0);
        assert!(curve.samples[135] > 1.0);
        assert!((curve.samples[136] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn combines_previous_and_current_histories() {
        let previous = [1.0f32; GAIN_HISTORY_SLOTS];
        let current = [2.0f32; GAIN_HISTORY_SLOTS];
        let combined = combined_gain_history(&previous, &current);
        assert_eq!(combined[..GAIN_HISTORY_SLOTS], previous);
        assert_eq!(combined[GAIN_HISTORY_SLOTS..], current);
    }

    #[test]
    fn estimates_slot_envelope_from_band_samples() {
        let mut samples = vec![0.0f32; 256];
        samples[31] = -0.75;
        samples[200] = 0.5;
        let envelope = estimate_envelope_slots(&samples).unwrap();
        assert!((envelope[3] - 0.75).abs() < 1e-6);
        assert!((envelope[25] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn window_variants_match_extracted_zero_regions() {
        let full = decoder_window_table(DecoderWindowKind::Full);
        let zero_tail = decoder_window_table(DecoderWindowKind::ZeroTail);
        let zero_head = decoder_window_table(DecoderWindowKind::ZeroHead);
        let zero_edges = decoder_window_table(DecoderWindowKind::ZeroEdges);

        assert!(full.iter().all(|sample| *sample > 0.0));
        assert!(
            zero_tail[GAIN_CURVE_SAMPLES - WINDOW_EDGE_ZERO_SAMPLES..]
                .iter()
                .all(|sample| *sample == 0.0)
        );
        assert!(
            zero_head[..WINDOW_EDGE_ZERO_SAMPLES]
                .iter()
                .all(|sample| *sample == 0.0)
        );
        assert!(
            zero_edges[..WINDOW_EDGE_ZERO_SAMPLES]
                .iter()
                .all(|sample| *sample == 0.0)
        );
        assert!(
            zero_edges[GAIN_CURVE_SAMPLES - WINDOW_EDGE_ZERO_SAMPLES..]
                .iter()
                .all(|sample| *sample == 0.0)
        );
    }

    #[test]
    fn gain_estimator_uses_previous_history_for_transition_points() {
        let mut current = [0.1f32; GAIN_HISTORY_SLOTS];
        let previous = [0.1f32; GAIN_HISTORY_SLOTS];
        current[5] = 1.0;
        current[9] = 0.7;
        current[12] = 0.5;
        let band = estimate_gain_band(&current, &previous, 0, 0.1);
        assert!(!band.points.is_empty());
        assert!(matches!(band.points[0].location, 4..=6));
    }

    #[test]
    fn gain_estimator_detects_current_frame_onset() {
        let mut current = [0.1f32; GAIN_HISTORY_SLOTS];
        let previous = [0.1f32; GAIN_HISTORY_SLOTS];
        current[5] = 1.0;
        current[9] = 0.8;
        current[12] = 0.6;
        let band = estimate_gain_band(&current, &previous, 0, 0.1);
        assert!(!band.points.is_empty());
        assert!(band.points[0].location <= 12);
    }
}
