use anyhow::{Result, bail, ensure};
use hound::{SampleFormat, WavReader};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct WavData {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl WavData {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    pub fn channel_samples(&self, channel: usize) -> Result<Vec<f32>> {
        ensure!(
            channel < self.channels as usize,
            "channel {} out of range",
            channel
        );
        Ok(self
            .samples
            .chunks_exact(self.channels as usize)
            .map(|frame| frame[channel])
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct CompareMetrics {
    pub compared_samples: usize,
    pub compared_frames: usize,
    pub reference_peak_dbfs: f64,
    pub candidate_peak_dbfs: f64,
    pub normalization_gain_db: f64,
    pub snr_db: f64,
    pub rmse: f64,
    pub mean_abs_error: f64,
    pub max_abs_error: f64,
    pub transient_count: usize,
    pub average_pre_echo_proxy_db: f64,
    pub worst_pre_echo_proxy_db: f64,
}

pub fn read_wav(path: &Path) -> Result<WavData> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();

    let samples = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Int, 16) => reader
            .samples::<i16>()
            .map(|sample| sample.map(|v| v as f32 / i16::MAX as f32))
            .collect::<std::result::Result<Vec<_>, _>>()?,
        (SampleFormat::Int, 24) | (SampleFormat::Int, 32) => {
            let scale = (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|v| v as f32 / scale))
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
        (SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        _ => bail!(
            "unsupported WAV format: {:?} {} bits",
            spec.sample_format,
            spec.bits_per_sample
        ),
    };

    Ok(WavData {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        samples,
    })
}

pub fn compare_wavs(reference: &WavData, candidate: &WavData) -> Result<CompareMetrics> {
    ensure!(
        reference.sample_rate == candidate.sample_rate,
        "sample rate mismatch: {} vs {}",
        reference.sample_rate,
        candidate.sample_rate
    );
    ensure!(
        reference.channels == candidate.channels,
        "channel count mismatch: {} vs {}",
        reference.channels,
        candidate.channels
    );

    let len = reference.samples.len().min(candidate.samples.len());
    ensure!(len > 0, "no samples to compare");

    let reference_slice = &reference.samples[..len];
    let candidate_slice = &candidate.samples[..len];

    let candidate_energy = candidate_slice
        .iter()
        .map(|sample| (*sample as f64) * (*sample as f64))
        .sum::<f64>();
    let cross = reference_slice
        .iter()
        .zip(candidate_slice.iter())
        .map(|(lhs, rhs)| (*lhs as f64) * (*rhs as f64))
        .sum::<f64>();

    let gain = if candidate_energy > 1e-18 {
        cross / candidate_energy
    } else {
        1.0
    };

    let reference_peak = reference_slice
        .iter()
        .map(|sample| sample.abs() as f64)
        .fold(0.0f64, f64::max);
    let candidate_peak = candidate_slice
        .iter()
        .map(|sample| (gain * *sample as f64).abs())
        .fold(0.0f64, f64::max);

    let mut signal_energy = 0.0f64;
    let mut error_energy = 0.0f64;
    let mut abs_error = 0.0f64;
    let mut max_abs_error = 0.0f64;

    for (reference_sample, candidate_sample) in reference_slice.iter().zip(candidate_slice.iter()) {
        let reference_sample = *reference_sample as f64;
        let candidate_sample = gain * *candidate_sample as f64;
        let error = reference_sample - candidate_sample;

        signal_energy += reference_sample * reference_sample;
        error_energy += error * error;
        abs_error += error.abs();
        max_abs_error = max_abs_error.max(error.abs());
    }

    let compared_frames = len / reference.channels as usize;
    let normalization_gain_db = 20.0 * (gain.abs().max(1e-18)).log10();
    let snr_db = 10.0 * (signal_energy.max(1e-18) / error_energy.max(1e-18)).log10();
    let rmse = (error_energy / len as f64).sqrt();
    let mean_abs_error = abs_error / len as f64;

    let transient_metrics = pre_echo_proxy(
        reference_slice,
        candidate_slice,
        gain,
        reference.channels,
        reference.sample_rate,
    );

    Ok(CompareMetrics {
        compared_samples: len,
        compared_frames,
        reference_peak_dbfs: 20.0 * reference_peak.max(1e-18).log10(),
        candidate_peak_dbfs: 20.0 * candidate_peak.max(1e-18).log10(),
        normalization_gain_db,
        snr_db,
        rmse,
        mean_abs_error,
        max_abs_error,
        transient_count: transient_metrics.count,
        average_pre_echo_proxy_db: transient_metrics.average_db,
        worst_pre_echo_proxy_db: transient_metrics.worst_db,
    })
}

#[derive(Debug, Clone, Copy)]
struct PreEchoProxy {
    count: usize,
    average_db: f64,
    worst_db: f64,
}

fn pre_echo_proxy(
    reference: &[f32],
    candidate: &[f32],
    gain: f64,
    channels: u16,
    sample_rate: u32,
) -> PreEchoProxy {
    let channel_count = channels as usize;
    let window_frames = ((sample_rate as usize) / 200).max(32);
    let window = window_frames * channel_count;

    if reference.len() < window * 3 {
        return PreEchoProxy {
            count: 0,
            average_db: 0.0,
            worst_db: 0.0,
        };
    }

    let mut energy = Vec::new();
    let mut offset = 0usize;
    while offset + window <= reference.len() {
        let block = &reference[offset..offset + window];
        let block_energy = block
            .iter()
            .map(|sample| (*sample as f64) * (*sample as f64))
            .sum::<f64>()
            / window as f64;
        energy.push((offset, block_energy));
        offset += window;
    }

    let mut attacks = Vec::new();
    for index in 1..energy.len() {
        let previous = energy[index - 1].1.max(1e-12);
        let current = energy[index].1;
        if current > previous * 4.0 && current > 1e-6 {
            attacks.push(energy[index].0);
        }
    }

    if attacks.is_empty() {
        return PreEchoProxy {
            count: 0,
            average_db: 0.0,
            worst_db: 0.0,
        };
    }

    let mut values = Vec::with_capacity(attacks.len());
    for attack in attacks {
        let pre_start = attack.saturating_sub(window);
        let pre_end = attack;
        let post_start = attack;
        let post_end = (attack + window).min(reference.len());

        let pre_error = squared_error(
            &reference[pre_start..pre_end],
            &candidate[pre_start..pre_end],
            gain,
        );
        let post_error = squared_error(
            &reference[post_start..post_end],
            &candidate[post_start..post_end],
            gain,
        );

        values.push(10.0 * ((pre_error + 1e-18) / (post_error + 1e-18)).log10());
    }

    let average_db = values.iter().sum::<f64>() / values.len() as f64;
    let worst_db = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    PreEchoProxy {
        count: values.len(),
        average_db,
        worst_db,
    }
}

fn squared_error(reference: &[f32], candidate: &[f32], gain: f64) -> f64 {
    reference
        .iter()
        .zip(candidate.iter())
        .map(|(lhs, rhs)| {
            let delta = *lhs as f64 - gain * *rhs as f64;
            delta * delta
        })
        .sum::<f64>()
}
