use anyhow::Result;
use atrac3_rs::atrac3::container::{
    Atrac3Bitrate, Atrac3ContainerOptions, wrap_prototype_in_riff_at3,
};
use atrac3_rs::atrac3::inspect::{
    band_window_kinds, ensure_channel_slots, extract_channel_slot, format_window_kind,
    parse_channel_prefix, parse_prefixes_for_channel, parse_riff_atrac3, summarize_gain_activity,
};
use atrac3_rs::atrac3::prototype::{PrototypeEncoder, PrototypeOptions};
use atrac3_rs::atrac3::qmf::FourBandQmf;
use atrac3_rs::atrac3::sound_unit::CodingMode;
use atrac3_rs::atrac3::synthesis::Atrac3Synthesis;
use atrac3_rs::metrics::{WavData, compare_wavs, read_wav};
use atrac3_rs::oracle::{OracleConfig, ReferenceEncode, run_oracle};
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Parser)]
#[command(author, version, about = "ATRAC3 comparison and analysis helpers")]
struct Cli {
    #[command(subcommand)]
    command: CommandSet,
}

#[derive(Subcommand)]
enum CommandSet {
    Oracle {
        #[arg(long)]
        tool: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        bitrate: Option<u32>,
        #[arg(long)]
        loop_start: Option<u32>,
        #[arg(long)]
        loop_end: Option<u32>,
        #[arg(long, default_value_t = false)]
        whole_loop: bool,
        #[arg(long)]
        decoded_dir: Option<PathBuf>,
    },
    CompareWav {
        #[arg(long)]
        reference: PathBuf,
        #[arg(long)]
        candidate: PathBuf,
    },
    QmfPreview {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = 0)]
        channel: usize,
    },
    TransformRoundtrip {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = 1)]
        frames: usize,
        #[arg(long, default_value_t = 0)]
        start_frame: usize,
        #[arg(long, default_value_t = 0)]
        warmup_frames: usize,
    },
    ProtoEncode {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1)]
        frames: usize,
        #[arg(long, default_value_t = 0)]
        start_frame: usize,
        #[arg(long, default_value_t = 0)]
        flush_frames: usize,
        #[arg(long)]
        target_bits: Option<usize>,
        #[arg(long, value_enum, default_value_t = CliCodingMode::Clc)]
        coding_mode: CliCodingMode,
        #[arg(long, default_value_t = 0.0001)]
        lambda: f32,
    },
    ProtoAt3 {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1)]
        frames: usize,
        #[arg(long, default_value_t = 0)]
        start_frame: usize,
        #[arg(long, default_value_t = 0)]
        flush_frames: usize,
        #[arg(long)]
        target_bits: Option<usize>,
        #[arg(long, value_enum, default_value_t = CliCodingMode::Clc)]
        coding_mode: CliCodingMode,
        #[arg(long, default_value_t = 0.0001)]
        lambda: f32,
        #[arg(long, value_enum)]
        bitrate: Option<CliAtrac3Bitrate>,
    },
    ProtoGainDump {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = 1)]
        frames: usize,
        #[arg(long, default_value_t = 0)]
        start_frame: usize,
    },
    InspectAt3 {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = 8)]
        frames: usize,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CliCodingMode {
    Clc,
    Vlc,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum CliAtrac3Bitrate {
    K66,
    K105,
    K132,
}

impl From<CliCodingMode> for CodingMode {
    fn from(value: CliCodingMode) -> Self {
        match value {
            CliCodingMode::Clc => CodingMode::Clc,
            CliCodingMode::Vlc => CodingMode::Vlc,
        }
    }
}

impl From<CliAtrac3Bitrate> for Atrac3Bitrate {
    fn from(value: CliAtrac3Bitrate) -> Self {
        match value {
            CliAtrac3Bitrate::K66 => Atrac3Bitrate::Kbps66,
            CliAtrac3Bitrate::K105 => Atrac3Bitrate::Kbps105,
            CliAtrac3Bitrate::K132 => Atrac3Bitrate::Kbps132,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        CommandSet::Oracle {
            tool,
            candidate,
            reference,
            source,
            bitrate,
            loop_start,
            loop_end,
            whole_loop,
            decoded_dir,
        } => {
            let reference_encode = match (source.as_ref(), bitrate) {
                (Some(_), Some(br)) => Some(ReferenceEncode {
                    bitrate_kbps: br,
                    loop_start,
                    loop_end,
                    whole_loop,
                }),
                _ => None,
            };

            let result = run_oracle(&OracleConfig {
                tool_path: tool,
                source_wav: source,
                candidate_at3: candidate,
                reference_at3: reference,
                reference_encode,
                decoded_dir,
            })?;

            println!("reference_decoded={}", result.reference_decoded.display());
            println!("candidate_decoded={}", result.candidate_decoded.display());
            print_metrics(&result.metrics);
        }
        CommandSet::CompareWav {
            reference,
            candidate,
        } => {
            let reference = read_wav(&reference)?;
            let candidate = read_wav(&candidate)?;
            let metrics = compare_wavs(&reference, &candidate)?;
            print_metrics(&metrics);
        }
        CommandSet::QmfPreview { input, channel } => {
            let wav = read_wav(&input)?;
            let samples = wav.channel_samples(channel)?;
            if samples.len() < 1024 {
                anyhow::bail!("input channel contains fewer than 1024 samples");
            }

            let mut qmf = FourBandQmf::default();
            let bands = qmf.split_frame(&samples[..1024])?;
            for (index, band) in bands.iter().enumerate() {
                let rms = (band
                    .iter()
                    .map(|sample| (*sample as f64) * (*sample as f64))
                    .sum::<f64>()
                    / band.len() as f64)
                    .sqrt();
                let peak = band
                    .iter()
                    .map(|sample| sample.abs() as f64)
                    .fold(0.0f64, f64::max);
                println!("band{}: rms={:.9} peak={:.9}", index, rms, peak);
            }
        }
        CommandSet::TransformRoundtrip {
            input,
            frames,
            start_frame,
            warmup_frames,
        } => {
            let wav = read_wav(&input)?;
            let channel_count = wav.channels as usize;
            let total_pcm_frames = wav.frames();
            let start_sample = start_frame * 1024;
            anyhow::ensure!(
                start_sample <= total_pcm_frames,
                "start_frame {} exceeds available PCM frames {}",
                start_frame,
                total_pcm_frames
            );

            let available_frames = total_pcm_frames.saturating_sub(start_sample).div_ceil(1024);
            let frame_count = frames.min(available_frames);
            anyhow::ensure!(frame_count > 0, "no ATRAC3 frames available for roundtrip");
            anyhow::ensure!(
                warmup_frames < frame_count,
                "warmup_frames {} must be smaller than frame_count {}",
                warmup_frames,
                frame_count
            );

            let channels = (0..channel_count)
                .map(|channel| wav.channel_samples(channel))
                .collect::<Result<Vec<_>>>()?;

            let mut encoder = PrototypeEncoder::new(channel_count);
            let mut synthesis = Atrac3Synthesis::new(channel_count);
            // The MDCT overlap-add has a 1-frame delay: reconstruction at
            // frame t corresponds to the input at frame t-1.  We store all
            // input frames and align them accordingly when building the
            // comparison vectors.
            let mut all_inputs: Vec<Vec<Vec<f32>>> = Vec::with_capacity(frame_count);
            let mut all_reconstructed: Vec<Vec<Vec<f32>>> = Vec::with_capacity(frame_count);

            for frame_index in 0..frame_count {
                let start = start_sample + frame_index * 1024;
                let frame_channels = channels
                    .iter()
                    .map(|channel| slice_frame(channel, start))
                    .collect::<Vec<_>>();
                let coefficients = encoder.analyze_frame_coefficients(
                    &frame_channels
                        .iter()
                        .map(|channel| channel.as_slice())
                        .collect::<Vec<_>>(),
                )?;
                let reconstructed = synthesis.synthesize_frame(
                    &coefficients
                        .iter()
                        .map(|channel| channel.as_slice())
                        .collect::<Vec<_>>(),
                )?;

                all_inputs.push(frame_channels);
                all_reconstructed.push(reconstructed);
            }

            let effective_warmup = warmup_frames.max(1);
            let mut reference_samples = Vec::with_capacity(
                (frame_count - effective_warmup) * 1024 * channel_count,
            );
            let mut candidate_samples = Vec::with_capacity(
                (frame_count - effective_warmup) * 1024 * channel_count,
            );

            for frame_index in effective_warmup..frame_count {
                let input_frame = &all_inputs[frame_index - 1];
                let recon_frame = &all_reconstructed[frame_index];
                for sample_index in 0..1024 {
                    for channel_index in 0..channel_count {
                        reference_samples
                            .push(input_frame[channel_index][sample_index]);
                        candidate_samples
                            .push(recon_frame[channel_index][sample_index]);
                    }
                }
            }

            let reference = WavData {
                sample_rate: wav.sample_rate,
                channels: wav.channels,
                samples: reference_samples,
            };
            let candidate = WavData {
                sample_rate: wav.sample_rate,
                channels: wav.channels,
                samples: candidate_samples,
            };
            let metrics = compare_wavs(&reference, &candidate)?;
            println!("roundtrip_frames={}", frame_count);
            println!("warmup_frames={}", warmup_frames);
            print_metrics(&metrics);
        }
        CommandSet::ProtoEncode {
            input,
            output,
            frames,
            start_frame,
            flush_frames,
            target_bits,
            coding_mode,
            lambda,
        } => {
            let wav = read_wav(&input)?;
            let result = PrototypeEncoder::encode_wav(
                &wav,
                PrototypeOptions {
                    coding_mode: coding_mode.into(),
                    lambda,
                    frame_limit: Some(frames),
                    start_frame,
                    flush_frames,
                    target_bits_per_channel: target_bits,
                },
            )?;

            if let Some(parent) = output.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&output, &result.bytes)?;
            println!("output={}", output.display());
            println!("sample_rate={}", result.sample_rate);
            println!("channels={}", result.channel_count);
            println!("frames={}", result.frame_count);
            println!("raw_bytes={}", result.bytes.len());
            for (frame_index, frame) in result.frames.iter().enumerate() {
                println!(
                    "frame{}: bits={} bytes={} channels={}",
                    frame_index,
                    frame.bit_len,
                    frame.bytes.len(),
                    frame.channels.len()
                );
                for (channel_index, channel) in frame.channels.iter().enumerate() {
                    let coded_subbands = channel.sound_unit.spectrum.subbands.len();
                    let coded_qmf_bands = channel.sound_unit.coded_qmf_bands;
                    let mse = channel.spectrum.mse;
                    let payload_bits = channel.spectrum.payload_bits;
                    println!(
                        "frame{}.ch{}: bits={} coded_qmf_bands={} coded_subbands={} payload_bits={} mse={:.8}",
                        frame_index,
                        channel_index,
                        channel.bit_len,
                        coded_qmf_bands,
                        coded_subbands,
                        payload_bits,
                        mse
                    );
                }
            }
        }
        CommandSet::ProtoAt3 {
            input,
            output,
            frames,
            start_frame,
            flush_frames,
            target_bits,
            coding_mode,
            lambda,
            bitrate,
        } => {
            let wav = read_wav(&input)?;
            let channel_count = wav.channels as u16;
            let allocator_target_bits = target_bits.or_else(|| {
                bitrate.map(|bitrate| {
                    Atrac3Bitrate::from(bitrate).block_align(channel_count) as usize * 8
                        / channel_count as usize
                })
            });
            let encoded = PrototypeEncoder::encode_wav(
                &wav,
                PrototypeOptions {
                    coding_mode: coding_mode.into(),
                    lambda,
                    frame_limit: Some(frames),
                    start_frame,
                    flush_frames,
                    target_bits_per_channel: allocator_target_bits,
                },
            )?;
            let at3 = wrap_prototype_in_riff_at3(
                &encoded,
                Atrac3ContainerOptions {
                    bitrate: bitrate.map(Into::into),
                },
            )?;

            if let Some(parent) = output.parent() {
                if !parent.as_os_str().is_empty() {
                    fs::create_dir_all(parent)?;
                }
            }
            fs::write(&output, &at3.bytes)?;
            println!("output={}", output.display());
            println!("sample_rate={}", encoded.sample_rate);
            println!("channels={}", encoded.channel_count);
            println!("frames={}", encoded.frame_count);
            println!(
                "bitrate_kbps={}",
                at3.bitrate.kbps(encoded.channel_count as u16)
            );
            println!("block_align={}", at3.block_align);
            println!("avg_bytes_per_sec={}", at3.avg_bytes_per_sec);
            println!("file_bytes={}", at3.bytes.len());
        }
        CommandSet::ProtoGainDump {
            input,
            frames,
            start_frame,
        } => {
            let wav = read_wav(&input)?;
            let channel_count = wav.channels as usize;
            let total_pcm_frames = wav.frames();
            let start_sample = start_frame * 1024;
            anyhow::ensure!(
                start_sample <= total_pcm_frames,
                "start_frame {} exceeds available PCM frames {}",
                start_frame,
                total_pcm_frames
            );
            let available_frames = total_pcm_frames.saturating_sub(start_sample).div_ceil(1024);
            let frame_count = frames.min(available_frames);
            anyhow::ensure!(frame_count > 0, "no ATRAC3 frames available for gain dump");

            let channels = (0..channel_count)
                .map(|channel| wav.channel_samples(channel))
                .collect::<Result<Vec<_>>>()?;
            let mut encoder = PrototypeEncoder::new(channel_count);

            for frame_index in 0..frame_count {
                let start = start_sample + frame_index * 1024;
                let frame_channels = channels
                    .iter()
                    .map(|channel| slice_frame(channel, start))
                    .collect::<Vec<_>>();
                let inspected = encoder.inspect_gain_frame(
                    &frame_channels
                        .iter()
                        .map(|channel| channel.as_slice())
                        .collect::<Vec<_>>(),
                )?;

                for (channel_index, channel) in inspected.iter().enumerate() {
                    for (band_index, band) in channel.bands.iter().enumerate() {
                        let current_peak =
                            band.current_envelope.iter().copied().fold(0.0f32, f32::max);
                        let previous_peak = band
                            .previous_envelope
                            .iter()
                            .copied()
                            .fold(0.0f32, f32::max);
                        let peak_slot = band
                            .current_envelope
                            .iter()
                            .copied()
                            .enumerate()
                            .max_by(|left, right| left.1.partial_cmp(&right.1).unwrap())
                            .map(|(slot, _)| slot)
                            .unwrap_or(0);
                        let points = if band.gain_band.points.is_empty() {
                            "-".to_string()
                        } else {
                            band.gain_band
                                .points
                                .iter()
                                .map(|point| format!("{}@{}", point.level, point.location))
                                .collect::<Vec<_>>()
                                .join(" ")
                        };
                        let current_slots = band
                            .current_envelope
                            .iter()
                            .map(|value| format!("{:.6}", value))
                            .collect::<Vec<_>>()
                            .join(",");
                        let previous_slots = band
                            .previous_envelope
                            .iter()
                            .map(|value| format!("{:.6}", value))
                            .collect::<Vec<_>>()
                            .join(",");
                        println!(
                            "frame{}.ch{}.band{}: prev_peak={:.6} cur_peak={:.6} peak_slot={} points={}",
                            start_frame + frame_index,
                            channel_index,
                            band_index,
                            previous_peak,
                            current_peak,
                            peak_slot,
                            points
                        );
                        println!(
                            "frame{}.ch{}.band{}.current=[{}]",
                            start_frame + frame_index,
                            channel_index,
                            band_index,
                            current_slots
                        );
                        println!(
                            "frame{}.ch{}.band{}.previous=[{}]",
                            start_frame + frame_index,
                            channel_index,
                            band_index,
                            previous_slots
                        );
                    }
                }
            }
        }
        CommandSet::InspectAt3 { input, frames } => {
            let bytes = fs::read(&input)?;
            let container = parse_riff_atrac3(&bytes)?;
            ensure_channel_slots(&container)?;

            println!("input={}", input.display());
            println!("channels={}", container.channels);
            println!("block_align={}", container.block_align);
            println!("frames={}", container.frame_count);

            for channel_index in 0..container.channels as usize {
                let prefixes = parse_prefixes_for_channel(&bytes, &container, channel_index)?;
                let summary = summarize_gain_activity(&prefixes);
                for (band_index, (active_frames, total_points)) in summary.iter().enumerate() {
                    println!(
                        "channel{}.band{}: active_frames={} total_points={}",
                        channel_index, band_index, active_frames, total_points
                    );
                }

                let preview_frames = frames.min(prefixes.len());
                for frame_index in 0..preview_frames {
                    let prefix = &prefixes[frame_index];
                    let previous = frame_index.checked_sub(1).map(|index| &prefixes[index]);
                    let window_kinds = band_window_kinds(previous, prefix);
                    let slot =
                        extract_channel_slot(&bytes, &container, frame_index, channel_index)?;
                    let parsed = parse_channel_prefix(slot)?;
                    let gain_counts = parsed
                        .gain_bands
                        .iter()
                        .map(|band| band.len().to_string())
                        .collect::<Vec<_>>()
                        .join(",");
                    let window_desc = window_kinds
                        .iter()
                        .map(|kind| format_window_kind(*kind))
                        .collect::<Vec<_>>()
                        .join(",");
                    println!(
                        "frame{}.ch{}: coded_qmf_bands={} tonal_components={} gain_counts=[{}] windows=[{}] bits={}",
                        frame_index,
                        channel_index,
                        parsed.coded_qmf_bands,
                        parsed.tonal_component_count,
                        gain_counts,
                        window_desc,
                        parsed.consumed_bits
                    );
                    for (band_index, band) in parsed.gain_bands.iter().enumerate() {
                        if band.is_empty() {
                            continue;
                        }
                        let points = band
                            .iter()
                            .map(|point| format!("{}@{}", point.level, point.location))
                            .collect::<Vec<_>>()
                            .join(" ");
                        println!(
                            "frame{}.ch{}.band{}: {}",
                            frame_index, channel_index, band_index, points
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

fn print_metrics(metrics: &atrac3_rs::metrics::CompareMetrics) {
    println!("compared_samples={}", metrics.compared_samples);
    println!("compared_frames={}", metrics.compared_frames);
    println!("reference_peak_dbfs={:.4}", metrics.reference_peak_dbfs);
    println!("candidate_peak_dbfs={:.4}", metrics.candidate_peak_dbfs);
    println!("normalization_gain_db={:.4}", metrics.normalization_gain_db);
    println!("snr_db={:.4}", metrics.snr_db);
    println!("rmse={:.8}", metrics.rmse);
    println!("mean_abs_error={:.8}", metrics.mean_abs_error);
    println!("max_abs_error={:.8}", metrics.max_abs_error);
    println!("transient_count={}", metrics.transient_count);
    println!(
        "pre_echo_proxy_avg_db={:.4}",
        metrics.average_pre_echo_proxy_db
    );
    println!(
        "pre_echo_proxy_worst_db={:.4}",
        metrics.worst_pre_echo_proxy_db
    );
}

fn slice_frame(channel: &[f32], start: usize) -> Vec<f32> {
    let mut frame = vec![0.0f32; 1024];
    let shifted_start = start as isize + analysis_sample_offset();
    for (dst_index, sample) in frame.iter_mut().enumerate() {
        let src_index = shifted_start + dst_index as isize;
        if (0..channel.len() as isize).contains(&src_index) {
            *sample = channel[src_index as usize];
        }
    }
    frame
}

fn analysis_sample_offset() -> isize {
    static VALUE: OnceLock<isize> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_ANALYSIS_SAMPLE_OFFSET")
            .ok()
            .and_then(|value| value.parse::<isize>().ok())
            .unwrap_or(0)
    })
}
