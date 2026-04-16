use anyhow::{Result, ensure};
use std::sync::OnceLock;

use super::{
    SAMPLES_PER_FRAME,
    bitstream::BitWriter,
    gain::{GAIN_HISTORY_SLOTS, build_gain_curve, estimate_gain_band},
    mdct::{MDCT_COEFFS_PER_BAND, MDCT_INPUT_SAMPLES, Mdct256},
    qmf::{FourBandQmf, estimate_envelopes_from_interleaved},
    quant::{
        self, ATRAC3_SUBBAND_TAB, SearchOptions, SpectrumEncoding,
        build_basic_sound_unit_from_encoding, build_spectral_unit,
    },
    sound_unit::{ChannelSoundUnit, CodingMode, GainBand, RawBitPayload, SpectralSubband},
};
use crate::metrics::WavData;

const DEFAULT_QUANTIZER_COMPAT_GAIN: f32 = 7500.0;
const DEFAULT_APPLY_ODD_BAND_REVERSE: bool = true;
const DEFAULT_APPLY_GAIN_ESTIMATION: bool = false;
const DEFAULT_ANALYSIS_SAMPLE_OFFSET: isize = 69;
const DEFAULT_USE_REFERENCE_MDCT: bool = false;
const QMF_BAND_COUNT: usize = 4;

fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(default)
}

fn env_choice(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn quantizer_compat_gain() -> f32 {
    static VALUE: OnceLock<f32> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_QUANT_GAIN")
            .ok()
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite() && *value > 0.0)
            .unwrap_or(DEFAULT_QUANTIZER_COMPAT_GAIN)
    })
}

fn apply_odd_band_reverse() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("ATRAC3_ODD_REVERSE", DEFAULT_APPLY_ODD_BAND_REVERSE))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GainMode { Off, Legacy, Modern }

fn gain_mode() -> GainMode {
    static VALUE: OnceLock<GainMode> = OnceLock::new();
    *VALUE.get_or_init(|| match env_choice("ATRAC3_GAIN").as_deref() {
        Some("0" | "false" | "no" | "off") => GainMode::Off,
        Some("1" | "true" | "yes" | "on" | "legacy") => GainMode::Legacy,
        Some("modern") => GainMode::Modern,
        _ => GainMode::Off, // Gain-Points deaktiviert — Pre-Echo via MDCT-Fading statt
    })
}

fn apply_gain_estimation() -> bool {
    gain_mode() != GainMode::Off
}

fn analysis_sample_offset() -> isize {
    static VALUE: OnceLock<isize> = OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("ATRAC3_ANALYSIS_SAMPLE_OFFSET")
            .ok()
            .and_then(|value| value.parse::<isize>().ok())
            .unwrap_or(DEFAULT_ANALYSIS_SAMPLE_OFFSET)
    })
}

fn swap_gain_curve_order() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("ATRAC3_GAIN_CURVE_SWAP", false))
}

fn use_reference_mdct() -> bool {
    static VALUE: OnceLock<bool> = OnceLock::new();
    *VALUE.get_or_init(|| env_flag("ATRAC3_REF_MDCT", DEFAULT_USE_REFERENCE_MDCT))
}

#[derive(Debug, Clone, Copy)]
enum MdctInputOrder {
    CurrentThenOverlap,
    OverlapThenCurrent,
}

impl MdctInputOrder {
    fn from_env() -> Self {
        match env_choice("ATRAC3_MDCT_INPUT_ORDER").as_deref() {
            Some("current-first" | "current_then_overlap") => Self::CurrentThenOverlap,
            Some("overlap-first" | "previous-first" | "overlap_then_current") => {
                Self::OverlapThenCurrent
            }
            _ => Self::OverlapThenCurrent,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct MdctInputVariant {
    order: MdctInputOrder,
    reverse_first: bool,
    reverse_second: bool,
    negate_first: bool,
    negate_second: bool,
}

impl MdctInputVariant {
    fn from_env() -> Self {
        Self {
            order: MdctInputOrder::from_env(),
            reverse_first: env_flag("ATRAC3_MDCT_REVERSE_FIRST", false),
            reverse_second: env_flag("ATRAC3_MDCT_REVERSE_SECOND", false),
            negate_first: env_flag("ATRAC3_MDCT_NEGATE_FIRST", false),
            negate_second: env_flag("ATRAC3_MDCT_NEGATE_SECOND", false),
        }
    }

    fn build_input(
        &self,
        current: &[f32],
        overlap: &[f32],
    ) -> [f32; MDCT_INPUT_SAMPLES] {
        let mut current_block = [0.0f32; MDCT_COEFFS_PER_BAND];
        let mut overlap_block = [0.0f32; MDCT_COEFFS_PER_BAND];
        current_block.copy_from_slice(&current[..MDCT_COEFFS_PER_BAND]);
        overlap_block.copy_from_slice(&overlap[..MDCT_COEFFS_PER_BAND]);

        let (mut first, mut second) = match self.order {
            MdctInputOrder::CurrentThenOverlap => (current_block, overlap_block),
            MdctInputOrder::OverlapThenCurrent => (overlap_block, current_block),
        };

        if self.reverse_first {
            first.reverse();
        }
        if self.reverse_second {
            second.reverse();
        }
        if self.negate_first {
            for sample in &mut first {
                *sample = -*sample;
            }
        }
        if self.negate_second {
            for sample in &mut second {
                *sample = -*sample;
            }
        }

        let mut input = [0.0f32; MDCT_INPUT_SAMPLES];
        input[..MDCT_COEFFS_PER_BAND].copy_from_slice(&first);
        input[MDCT_COEFFS_PER_BAND..].copy_from_slice(&second);
        input
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PrototypeOptions {
    pub coding_mode: CodingMode,
    pub lambda: f32,
    pub frame_limit: Option<usize>,
    pub start_frame: usize,
    pub flush_frames: usize,
    pub target_bits_per_channel: Option<usize>,
}

impl Default for PrototypeOptions {
    fn default() -> Self {
        Self {
            coding_mode: CodingMode::Clc,
            lambda: 0.0001,
            frame_limit: Some(1),
            start_frame: 0,
            flush_frames: 0,
            target_bits_per_channel: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PrototypeFrameChannel {
    pub sound_unit: ChannelSoundUnit,
    pub spectrum: SpectrumEncoding,
    pub bit_len: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PrototypeFrame {
    pub channels: Vec<PrototypeFrameChannel>,
    pub bit_len: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PrototypeEncodeResult {
    pub sample_rate: u32,
    pub channel_count: usize,
    pub frame_count: usize,
    pub frames: Vec<PrototypeFrame>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct GainInspectionBand {
    pub current_envelope: [f32; GAIN_HISTORY_SLOTS],
    pub previous_envelope: [f32; GAIN_HISTORY_SLOTS],
    pub gain_band: GainBand,
}

#[derive(Debug, Clone)]
pub struct GainInspectionChannel {
    pub bands: Vec<GainInspectionBand>,
}

#[derive(Debug, Clone)]
struct AnalyzedChannel {
    coefficients: Vec<f32>,
    gain_bands: Vec<GainBand>,
}

pub struct PrototypeEncoder {
    qmf: Vec<FourBandQmf>,
    mdct: Mdct256,
    overlap: Vec<[[f32; MDCT_COEFFS_PER_BAND]; 4]>,
    previous_gain_bands: Vec<Vec<GainBand>>,
    previous_envelopes: Vec<[[f32; GAIN_HISTORY_SLOTS]; QMF_BAND_COUNT]>,
    previous_peak_state: Vec<[f32; QMF_BAND_COUNT]>,
    pending_analysis: Vec<AnalyzedChannel>,
}

impl PrototypeEncoder {
    pub fn new(channel_count: usize) -> Self {
        Self {
            qmf: vec![FourBandQmf::default(); channel_count],
            mdct: Mdct256::default(),
            overlap: vec![[[0.0; MDCT_COEFFS_PER_BAND]; 4]; channel_count],
            previous_gain_bands: vec![vec![GainBand::default(); QMF_BAND_COUNT]; channel_count],
            previous_envelopes: vec![[[0.0; GAIN_HISTORY_SLOTS]; QMF_BAND_COUNT]; channel_count],
            previous_peak_state: vec![[0.0; QMF_BAND_COUNT]; channel_count],
            pending_analysis: vec![zero_analyzed_channel(); channel_count],
        }
    }

    pub fn encode_wav(wav: &WavData, options: PrototypeOptions) -> Result<PrototypeEncodeResult> {
        let channel_count = wav.channels as usize;
        ensure!(channel_count > 0, "WAV must contain at least one channel");

        let total_pcm_frames = wav.frames();
        let start_sample = options.start_frame * SAMPLES_PER_FRAME;
        ensure!(
            start_sample <= total_pcm_frames,
            "start_frame {} exceeds available PCM frames {}",
            options.start_frame,
            total_pcm_frames
        );
        let remaining_pcm_frames = total_pcm_frames.saturating_sub(start_sample);
        let input_frames = remaining_pcm_frames.div_ceil(SAMPLES_PER_FRAME);
        let available_frames = input_frames + options.flush_frames;
        ensure!(available_frames > 0, "no ATRAC3 frames available to encode");
        let frame_count = options
            .frame_limit
            .unwrap_or(available_frames)
            .min(available_frames);

        let channels = (0..channel_count)
            .map(|channel| wav.channel_samples(channel))
            .collect::<Result<Vec<_>>>()?;

        let mut encoder = PrototypeEncoder::new(channel_count);
        // Phase 1: SERIAL analysis (QMF/MDCT have channel state).
        // Collect AnalyzedChannel for every frame.
        let mut all_analyses: Vec<Vec<AnalyzedChannel>> = Vec::with_capacity(frame_count);
        for frame_index in 0..frame_count {
            let start = start_sample + frame_index * SAMPLES_PER_FRAME;
            let frame_channels: Vec<Vec<f32>> = channels
                .iter()
                .map(|channel| slice_frame(channel, start))
                .collect();
            let analyses: Vec<AnalyzedChannel> = frame_channels
                .iter()
                .enumerate()
                .map(|(ch, samples)| encoder.analyze_channel_for_encoding(ch, samples))
                .collect::<Result<Vec<_>>>()?;
            encoder.pending_analysis = analyses.clone();
            all_analyses.push(analyses);
        }

        // ===== ECHTER 2-PASS ENCODING =====
        use rayon::prelude::*;
        let base_target = options.target_bits_per_channel.unwrap_or(1536);
        let search_opts = SearchOptions {
            lambda: options.lambda,
            target_bits: options.target_bits_per_channel,
            max_candidates_per_band: 64,
            tonal_marked_subbands: [false; 32],
        };

        // PASS 1: Encode mit Standard-Budget → sammle echte Metriken
        let pass1_frames: Vec<PrototypeFrame> = all_analyses
            .par_iter()
            .map(|analyses| {
                encoder.encode_analyzed_frame(analyses, options.coding_mode, search_opts)
            })
            .collect::<Result<Vec<_>>>()?;

        // ANALYSE: aus Pass 1 lernen — drei Insights pro Frame:
        // 1. Surplus: wie viele Bits wurden NICHT genutzt → Budget für Pass 2
        // 2. Pre-Echo: welche Frames haben Transient-Onset im Folge-Frame
        // 3. HF-Power-Ratio: wo produziert der Quantizer zu viel Noise
        struct FrameInsight {
            surplus_bits: usize,
            pre_echo_risk: bool,
            hf_noise_ratio: f32, // encoded_hf_power / original_hf_power
        }
        let insights: Vec<Vec<FrameInsight>> = pass1_frames
            .iter()
            .enumerate()
            .map(|(idx, frame)| {
                frame.channels.iter().enumerate().map(|(ch, channel)| {
                    let surplus = base_target.saturating_sub(channel.bit_len);
                    // Pre-Echo: nächster Frame > 8× lauter
                    let pre_echo_risk = if idx + 1 < all_analyses.len() {
                        let cur_e: f32 = all_analyses[idx][ch].coefficients[..512].iter()
                            .map(|c| c * c).sum();
                        let next_e: f32 = all_analyses[(idx + 1).min(all_analyses.len()-1)][ch]
                            .coefficients[..512].iter().map(|c| c * c).sum();
                        next_e > cur_e * 8.0
                    } else { false };
                    // HF Power Ratio: rekonstruierte HF vs Original-HF
                    let orig_hf: f32 = all_analyses[idx][ch].coefficients[512..].iter()
                        .map(|c| c * c).sum::<f32>().max(1e-20);
                    let enc_hf: f32 = channel.spectrum.reconstructed.get(512..)
                        .map(|s| s.iter().map(|c| c * c).sum()).unwrap_or(0.0f32);
                    let hf_noise_ratio = enc_hf / orig_hf;
                    FrameInsight { surplus_bits: surplus, pre_echo_risk, hf_noise_ratio }
                }).collect()
            })
            .collect();

        // PASS 2: Re-encode JEDES Frame mit Wissen aus Pass 1
        let frames: Vec<PrototypeFrame> = all_analyses
            .par_iter()
            .enumerate()
            .map(|(idx, analyses)| {
                let max_surplus = insights[idx].iter()
                    .map(|a| a.surplus_bits).max().unwrap_or(0);
                let any_pre_echo = insights[idx].iter().any(|a| a.pre_echo_risk);
                let max_hf_ratio = insights[idx].iter()
                    .map(|a| a.hf_noise_ratio).fold(0.0f32, f32::max);

                // Entscheide ob Pass 2 lohnt
                let needs_pass2 = max_surplus > 20
                    || any_pre_echo
                    || max_hf_ratio > 1.2; // HF zu laut → Floor-Sub aggressiver

                if needs_pass2 {
                    // Pass-2-optimierte Coefficients: Pre-Echo-Fading verstärken
                    // + HF-Floor-Subtraction anpassen basierend auf Pass-1-Ratio
                    let mut pass2_analyses: Vec<AnalyzedChannel> = analyses.clone();
                    for (ch, analysis) in pass2_analyses.iter_mut().enumerate() {
                        let insight = &insights[idx][ch];

                        // Pre-Echo: wenn nächster Frame laut wird, stärkeres Fading
                        // auf Band 0-1 im aktuellen Frame
                        if insight.pre_echo_risk {
                            for band_idx in 0..2 {
                                let s = band_idx * 256;
                                let e = s + 256;
                                if e > analysis.coefficients.len() { break; }
                                // Letzte 64 Coefs (= letzte 2 Slots) sanft faden
                                for i in (e-64)..e {
                                    let t = (e - i) as f32 / 64.0;
                                    analysis.coefficients[i] *= t * t;
                                }
                            }
                        }

                        // HF-Noise: wenn Pass 1 >120% HF-Power produziert hat,
                        // Floor-Subtraction in Pass 2 aggressiver machen
                        if insight.hf_noise_ratio > 1.2 {
                            let extra_alpha = (insight.hf_noise_ratio - 1.0).min(1.5);
                            for band in 20..32 {
                                let s = ATRAC3_SUBBAND_TAB[band];
                                let e = ATRAC3_SUBBAND_TAB[band + 1];
                                if e > analysis.coefficients.len() { break; }
                                let mut mags: Vec<f32> = analysis.coefficients[s..e].iter()
                                    .map(|c| c.abs()).collect();
                                mags.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                                let floor = mags[mags.len() / 4]; // 25th percentile
                                let threshold = floor * extra_alpha;
                                for c in analysis.coefficients[s..e].iter_mut() {
                                    let mag = c.abs();
                                    if mag <= threshold {
                                        *c = 0.0;
                                    } else {
                                        *c = c.signum() * (mag - threshold);
                                    }
                                }
                            }
                        }
                    }

                    let mut pass2_search = search_opts;
                    pass2_search.target_bits = Some(base_target + max_surplus / 3);

                    match encoder.encode_analyzed_frame(&pass2_analyses, options.coding_mode, pass2_search) {
                        Ok(f) if f.channels.iter().all(|ch| ch.bytes.len() <= 192) => Ok(f),
                        _ => Ok(pass1_frames[idx].clone()),
                    }
                } else {
                    Ok(pass1_frames[idx].clone())
                }
            })
            .collect::<Result<Vec<_>>>()?;

        let mut bytes = Vec::new();
        for f in &frames {
            bytes.extend_from_slice(&f.bytes);
        }

        Ok(PrototypeEncodeResult {
            sample_rate: wav.sample_rate,
            channel_count,
            frame_count,
            frames,
            bytes,
        })
    }

    pub fn encode_frame(
        &mut self,
        channels: &[&[f32]],
        coding_mode: CodingMode,
        search: SearchOptions,
    ) -> Result<PrototypeFrame> {
        ensure!(
            channels.len() == self.qmf.len(),
            "channel count mismatch: got {}, encoder expects {}",
            channels.len(),
            self.qmf.len()
        );

        let current_analysis = channels
            .iter()
            .enumerate()
            .map(|(channel_index, samples)| {
                ensure!(
                    samples.len() == SAMPLES_PER_FRAME,
                    "prototype encoder expects {} samples per channel frame, got {}",
                    SAMPLES_PER_FRAME,
                    samples.len()
                );
                self.analyze_channel_for_encoding(channel_index, samples)
            })
            .collect::<Result<Vec<_>>>()?;

        // Encode the current frame's analysis directly. The pending_analysis
        // buffer is kept for gain estimation state but the spectral data is
        // NOT delayed — the Sony pipeline analyzes, estimates gain, and
        // quantizes the same frame's data in a single pass.
        self.pending_analysis = current_analysis.clone();
        self.encode_analyzed_frame(&current_analysis, coding_mode, search)
    }

    fn encode_analyzed_frame(
        &self,
        analysis_channels: &[AnalyzedChannel],
        coding_mode: CodingMode,
        search: SearchOptions,
    ) -> Result<PrototypeFrame> {
        ensure!(
            analysis_channels.len() == self.qmf.len(),
            "analysis channel count mismatch: got {}, encoder expects {}",
            analysis_channels.len(),
            self.qmf.len()
        );

        let mut frame_channels = Vec::with_capacity(analysis_channels.len());
        let mut frame_writer = BitWriter::new();

        for analysis in analysis_channels {
            let gain_payload_bits = analysis
                .gain_bands
                .iter()
                .map(|band| band.points.len() * 9)
                .sum::<usize>();
            let min_subband_count =
                minimum_subband_count_for_target_bits(search.target_bits.unwrap_or_default());
            let padded_skip_bits = min_subband_count.saturating_sub(1) * 3;
            let base_target = search.target_bits.unwrap_or(1536);
            let spectral_budget = base_target.saturating_sub(gain_payload_bits + padded_skip_bits);

            // Tonal extraction: find prominent peaks, quantize them into
            // the tonal stream, and subtract the quantized tone from the
            // spectrum so the spectral allocator sees only the residual.
            // This avoids double-coding the same energy in both streams.
            let coded_qmf_target: u8 = 3;
            let mut residual = analysis.coefficients.clone();
            let tonal_result = quant::extract_tonal_components(
                &mut residual,
                spectral_budget,
                coded_qmf_target,
                coding_mode,
                4,
            )?;

            // HF Noise-Reduction: zwei Techniken kombiniert.
            //
            // 1. Source-Quality-Detection (128kbit MP3 erkennen):
            //    Wenn die Source oberhalb 16kHz quasi keine Energie hat
            //    (MP3 128kbit Low-Pass), wird Brilliance komplett genullt.
            //    Kein Sinn MP3-Noise zu encoden → Bits für Mid/Presence.
            //
            // 2. Brilliance Noise-Gate (Sony-Trick "weniger = natürlicher"):
            //    Schwache Brilliance-Coefs (< 10% Peak-Power) auf 0 setzen.
            //    Reduziert rekonstruierte Power von 121% auf ~65% (Sony-Level).
            let hf_start = 768; // Band 30 start (nur Brilliance)
            let hf_end = residual.len().min(1024);
            // Source-Detection: prüfe ob HF-Signal echt ist
            let hf_power: f32 = residual[hf_start..hf_end].iter()
                .map(|c| c * c).sum();
            let total_power: f32 = residual.iter().map(|c| c * c).sum();
            let hf_ratio = hf_power / (total_power + 1e-20);
            if hf_ratio < 0.001 {
                // MP3 128kbit oder ähnlich: kein echtes HF → komplett nullen
                for c in residual[hf_start..hf_end].iter_mut() { *c = 0.0; }
            } else {
                // Echtes HF vorhanden → band-adaptive Noise-Gate.
                // Presence (512-768): sanfter (15%) — enthält Stimmen/Harmonics
                // Brilliance (768-1024): aggressiver (8%) — Sony 64% Ziel
                // Nur Brilliance (768+) gaten. Presence enthält Stimmen.
                let mut peak_power: f32 = 0.0;
                for chunk in residual[hf_start..hf_end].chunks(4) {
                    let p: f32 = chunk.iter().map(|c| c * c).sum();
                    peak_power = peak_power.max(p);
                }
                if peak_power > 1e-12 {
                    let threshold = peak_power * 0.08;
                    for chunk in residual[hf_start..hf_end].chunks_mut(4) {
                        let p: f32 = chunk.iter().map(|c| c * c).sum();
                        if p < threshold {
                            for c in chunk.iter_mut() { *c = 0.0; }
                        }
                    }
                }
            }

            // Vorbis-Trick: Bark-Scale Psychoacoustic Masking Model.
            //
            // Berechne pro Band die Masking-Threshold: wie viel Noise wird
            // durch benachbarte Bänder verdeckt? Starkes LF-Signal maskiert
            // HF-Noise (forward masking über Bark-Scale). Das Ergebnis
            // bestimmt wie aggressiv der Spectral-Floor-Subtractor arbeitet:
            // - Hohe Masking → Noise wird verdeckt → aggressiver filtern (spart Bits)
            // - Niedrige Masking → Noise ist hörbar → sanfter filtern (erhält Signal)
            //
            // Simplified Bark-Scale Spreading: jedes Band breitet seine Energie
            // um 3 dB/Bark nach oben und 1.5 dB/Bark nach unten aus.
            let mut band_power = [0.0f32; 32];
            let mut masking_threshold = [0.0f32; 32];
            for band in 0..32 {
                let s = ATRAC3_SUBBAND_TAB[band];
                let e = ATRAC3_SUBBAND_TAB[band + 1];
                if e > residual.len() { break; }
                band_power[band] = residual[s..e].iter()
                    .map(|c| c * c).sum::<f32>() / (e - s).max(1) as f32;
            }
            // Forward masking: LF→HF spreading (dominanter Effekt)
            for band in 0..32 {
                let mut mask = band_power[band] * 0.001; // self-masking: -30 dB
                // Spread from lower bands (forward masking)
                for lower in 0..band {
                    let bark_dist = (band - lower) as f32 * 0.5; // ~0.5 Bark pro ATRAC3-Band
                    let spread = band_power[lower] * 10.0f32.powf(-0.3 * bark_dist); // 3 dB/Bark
                    mask = mask.max(spread * 0.001); // masking threshold = -30 dB below masker
                }
                masking_threshold[band] = mask;
            }

            // Spectral-Floor-Subtraction (klassisches Denoising):
            //
            // Pro HF-Band: schätze den Noise-Floor als Median der |Coefs|.
            // Subtrahiere den Floor von jedem Coef (Soft-Threshold).
            // Coefs unter dem Floor → 0. Coefs über dem Floor → Signal.
            //
            // Mathematisch: cleaned = sign(c) × max(0, |c| - floor × α)
            //
            // Das ist was Opus/Vorbis intern machen: "noise shaping" durch
            // Entfernung des flachen Noise-Bodens. Ergebnis: nur die echten
            // Peaks (Harmonics, Transienten) bleiben → weniger Mantissa=±1
            // Noise, höhere Korrelation, natürlicherer Sound.
            for band in 16..32 {
                let s = ATRAC3_SUBBAND_TAB[band];
                let e = ATRAC3_SUBBAND_TAB[band + 1];
                if e > residual.len() { break; }
                let width = e - s;
                if width < 4 { continue; }
                // Schätze Noise-Floor via sortierte Magnitude (Percentile-25)
                let mut mags: Vec<f32> = residual[s..e].iter()
                    .map(|c| c.abs()).collect();
                mags.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let floor_idx = width / 4; // 25th percentile
                let noise_floor = mags[floor_idx];
                if noise_floor < 1e-12 { continue; }
                // Masking-aware Threshold: wenn starkes Masking aktiv, aggressiver
                // filtern (Noise wird verdeckt). Ohne Masking: konservativ.
                let masking_ratio = if band_power[band] > 1e-20 {
                    (masking_threshold[band] / band_power[band]).sqrt().clamp(0.0, 1.0)
                } else { 0.0 };
                // Basis-Alpha + Masking-Boost
                let base_alpha = if band >= 28 { 1.0f32 }
                    else if band >= 22 { 0.5 }
                    else { 0.3 };
                let alpha = base_alpha + masking_ratio * 1.5; // stärker wenn maskiert
                let threshold = noise_floor * alpha;
                // Soft-Threshold Subtraction
                for coef in residual[s..e].iter_mut() {
                    let mag = coef.abs();
                    if mag <= threshold {
                        *coef = 0.0;
                    } else {
                        *coef = coef.signum() * (mag - threshold);
                    }
                }
            }

            let mut adjusted_search = search;
            adjusted_search.target_bits =
                Some(spectral_budget.saturating_sub(tonal_result.tonal_bits));
            adjusted_search.tonal_marked_subbands = tonal_result.tonal_subbands;

            let mut spectrum = build_spectral_unit(&residual, coding_mode, adjusted_search)?;
            pad_spectral_unit(&mut spectrum, min_subband_count);
            let mut sound_unit = build_basic_sound_unit_from_encoding(&spectrum);
            let coded_qmf_bands = sound_unit.coded_qmf_bands.min(coded_qmf_target) as usize;
            sound_unit.coded_qmf_bands = coded_qmf_bands as u8;
            sound_unit.gain_bands = analysis.gain_bands[..coded_qmf_bands].to_vec();

            // Attach tonal, resize band_flags/cells to match coded_qmf_bands
            sound_unit.tonal_mode_selector = tonal_result.tonal_mode_selector;
            sound_unit.tonal_components = tonal_result
                .tonal_components
                .into_iter()
                .map(|mut c| {
                    c.band_flags.resize(coded_qmf_bands, false);
                    c.cells.resize(
                        coded_qmf_bands * 4,
                        crate::atrac3::sound_unit::TonalCell::default(),
                    );
                    c
                })
                .collect();

            // Safety: if the total exceeds the per-channel slot, drop the
            // tonal and re-quantize the original spectrum with the full
            // budget. Spectral-only encoding is always the fallback.
            if let Ok(total) = sound_unit.bit_len() {
                if total > base_target {
                    sound_unit.tonal_mode_selector =
                        crate::atrac3::sound_unit::TonalCodingModeSelector::AllVlc;
                    sound_unit.tonal_components.clear();
                    adjusted_search.target_bits = Some(spectral_budget);
                    let s2 = build_spectral_unit(
                        &analysis.coefficients,
                        coding_mode,
                        adjusted_search,
                    )?;
                    sound_unit.spectrum = s2.spectral_unit;
                }
            }

            frame_channels.push(encode_channel(sound_unit, spectrum)?);
            frame_channels
                .last()
                .unwrap()
                .sound_unit
                .write_to(&mut frame_writer)?;
        }

        let frame_bits = frame_writer.bit_len();
        frame_writer.byte_align_zero();
        let frame_bytes = frame_writer.into_bytes();

        Ok(PrototypeFrame {
            channels: frame_channels,
            bit_len: frame_bits,
            bytes: frame_bytes,
        })
    }

    pub fn inspect_gain_frame(
        &mut self,
        channels: &[&[f32]],
    ) -> Result<Vec<GainInspectionChannel>> {
        ensure!(
            channels.len() == self.qmf.len(),
            "channel count mismatch: got {}, encoder expects {}",
            channels.len(),
            self.qmf.len()
        );

        let mut debug_channels = Vec::with_capacity(channels.len());
        for (channel_index, samples) in channels.iter().enumerate() {
            ensure!(
                samples.len() == SAMPLES_PER_FRAME,
                "prototype encoder expects {} samples per channel frame, got {}",
                SAMPLES_PER_FRAME,
                samples.len()
            );

            let frame = self.qmf[channel_index].split_frame_with_layout(samples)?;
            let envelopes = estimate_envelopes_from_interleaved(&frame.interleaved);
            let mut current_envelopes = [[0.0f32; GAIN_HISTORY_SLOTS]; QMF_BAND_COUNT];
            let mut current_gain_bands = vec![GainBand::default(); QMF_BAND_COUNT];
            let mut debug_bands = Vec::with_capacity(QMF_BAND_COUNT);

            for (band_index, _band_samples) in frame.bands.into_iter().enumerate() {
                let previous_envelope = self.previous_envelopes[channel_index][band_index];
                let current_envelope = envelopes[band_index];
                let history_peak_state = self.previous_peak_state[channel_index][band_index];
                let gain_band = if apply_gain_estimation() {
                    estimate_gain_band(
                        &current_envelope,
                        &previous_envelope,
                        band_index,
                        history_peak_state,
                    )
                } else {
                    GainBand::default()
                };

                current_envelopes[band_index] = current_envelope;
                current_gain_bands[band_index] = gain_band.clone();
                self.previous_peak_state[channel_index][band_index] =
                    previous_envelope.iter().copied().fold(0.0f32, f32::max);
                debug_bands.push(GainInspectionBand {
                    current_envelope,
                    previous_envelope,
                    gain_band,
                });
            }

            self.previous_envelopes[channel_index] = current_envelopes;
            self.previous_gain_bands[channel_index] = current_gain_bands;
            debug_channels.push(GainInspectionChannel { bands: debug_bands });
        }

        Ok(debug_channels)
    }

    pub fn analyze_frame_coefficients(&mut self, channels: &[&[f32]]) -> Result<Vec<Vec<f32>>> {
        ensure!(
            channels.len() == self.qmf.len(),
            "channel count mismatch: got {}, encoder expects {}",
            channels.len(),
            self.qmf.len()
        );

        channels
            .iter()
            .enumerate()
            .map(|(channel_index, samples)| {
                ensure!(
                    samples.len() == SAMPLES_PER_FRAME,
                    "prototype encoder expects {} samples per channel frame, got {}",
                    SAMPLES_PER_FRAME,
                    samples.len()
                );
                self.analyze_channel_raw(channel_index, samples)
            })
            .collect()
    }

    fn analyze_channel_for_encoding(
        &mut self,
        channel_index: usize,
        samples: &[f32],
    ) -> Result<AnalyzedChannel> {
        self.analyze_channel(channel_index, samples, true)
    }

    fn analyze_channel_raw(&mut self, channel_index: usize, samples: &[f32]) -> Result<Vec<f32>> {
        Ok(self
            .analyze_channel(channel_index, samples, false)?
            .coefficients)
    }

    fn analyze_channel(
        &mut self,
        channel_index: usize,
        samples: &[f32],
        encode_gain: bool,
    ) -> Result<AnalyzedChannel> {
        let frame = self.qmf[channel_index].split_frame_with_layout(samples)?;
        let envelope_slots = estimate_envelopes_from_interleaved(&frame.interleaved);
        let mut coefficients = vec![0.0f32; SAMPLES_PER_FRAME];
        let gain_enabled = encode_gain && apply_gain_estimation();
        let mut gain_bands = vec![GainBand::default(); QMF_BAND_COUNT];
        let mut envelopes = [[0.0f32; GAIN_HISTORY_SLOTS]; QMF_BAND_COUNT];

        for (band_index, band_samples) in frame.bands.into_iter().enumerate() {
            let envelope = envelope_slots[band_index];
            envelopes[band_index] = envelope;
            let history_peak_state = self.previous_peak_state[channel_index][band_index];

            let gain_band = match gain_mode() {
                GainMode::Legacy => estimate_gain_band(
                    &envelope,
                    &self.previous_envelopes[channel_index][band_index],
                    band_index,
                    history_peak_state,
                ),
                GainMode::Modern => {
                    use super::gain::modern_gain_estimation;
                    modern_gain_estimation(&envelope, band_index)
                }
                GainMode::Off => GainBand::default(),
            };
            gain_bands[band_index] = gain_band.clone();

            let analysis_samples = if gain_enabled {
                let curve = if swap_gain_curve_order() {
                    build_gain_curve(
                        &self.previous_gain_bands[channel_index][band_index],
                        &gain_band,
                    )?
                } else {
                    build_gain_curve(
                        &gain_band,
                        &self.previous_gain_bands[channel_index][band_index],
                    )?
                };
                compensate_band_samples(&band_samples, &curve.samples)
            } else {
                let mut out = [0.0f32; MDCT_COEFFS_PER_BAND];
                out.copy_from_slice(&band_samples[..MDCT_COEFFS_PER_BAND]);
                out
            };

            // Pre-Echo-Filter: MDCT-Input-Fading (LAME-inspiriert).
            // Statt Gain-Points (kosten Bits + Compensation zerstört MDCT)
            // faden wir bei erkannten Transienten die Pre-Onset-Samples
            // sanft auf den Overlap-Level. Kein Bit-Overhead, keine
            // Decoder-Abhängigkeit.
            let mut faded_samples = analysis_samples;
            if band_index < 2 {
                let mut slot_energy = [0.0f32; 8];
                for (s, chunk) in faded_samples.chunks(32).enumerate() {
                    if s >= 8 { break; }
                    slot_energy[s] = chunk.iter().map(|c| c * c).sum();
                }
                let mut running_max = slot_energy[0].max(1e-20);
                let mut best_ratio = 0.0f32;
                let mut onset_slot = 0usize;
                for k in 1..8 {
                    let ratio = slot_energy[k] / running_max;
                    if ratio > best_ratio { best_ratio = ratio; onset_slot = k; }
                    running_max = running_max.max(slot_energy[k]);
                }
                if best_ratio > 30.0 && onset_slot >= 2 {
                    let onset_sample = onset_slot * 32;
                    for i in 0..onset_sample {
                        let t = i as f32 / onset_sample as f32;
                        let fade = t * t; // quadratic fade-in: sanft am Anfang, schnell vorm Onset
                        faded_samples[i] *= fade;
                    }
                }
            }
            let mdct_input = MdctInputVariant::from_env().build_input(
                &faded_samples,
                &self.overlap[channel_index][band_index],
            );
            self.overlap[channel_index][band_index].copy_from_slice(&analysis_samples);

            let mut band_coefficients = if use_reference_mdct() {
                self.mdct.forward_reference(&mdct_input)
            } else {
                let mut c = self.mdct.forward(&mdct_input);
                // The reference transform scales by 1/128. Our allocator and scale factor
                // search are tuned against the unscaled coefficient range, so we compensate
                // back to that range here.
                for coefficient in &mut c {
                    *coefficient *= quantizer_compat_gain();
                }
                c
            };
            if apply_odd_band_reverse() && (band_index & 1 == 1) {
                band_coefficients.reverse();
            }

            let start = band_index * MDCT_COEFFS_PER_BAND;
            let end = start + MDCT_COEFFS_PER_BAND;
            coefficients[start..end].copy_from_slice(&band_coefficients);
        }

        if gain_enabled {
            for band_index in 0..QMF_BAND_COUNT {
                self.previous_peak_state[channel_index][band_index] = self.previous_envelopes
                    [channel_index][band_index]
                    .iter()
                    .copied()
                    .fold(0.0f32, f32::max);
            }
            self.previous_envelopes[channel_index] = envelopes;
            self.previous_gain_bands[channel_index] = gain_bands.clone();
        }

        Ok(AnalyzedChannel {
            coefficients,
            gain_bands,
        })
    }
}

fn compensate_band_samples(
    band_samples: &[f32],
    curve: &[f32; MDCT_COEFFS_PER_BAND],
) -> [f32; MDCT_COEFFS_PER_BAND] {
    let mut out = [0.0f32; MDCT_COEFFS_PER_BAND];
    for index in 0..MDCT_COEFFS_PER_BAND {
        let gain = curve[index];
        out[index] = if gain.abs() > 1e-9 {
            band_samples[index] / gain
        } else {
            band_samples[index]
        };
    }
    out
}

fn zero_analyzed_channel() -> AnalyzedChannel {
    AnalyzedChannel {
        coefficients: vec![0.0; SAMPLES_PER_FRAME],
        gain_bands: vec![GainBand::default(); QMF_BAND_COUNT],
    }
}

fn minimum_subband_count_for_target_bits(target_bits: usize) -> usize {
    if target_bits >= 1_536 {
        ATRAC3_SUBBAND_TAB
            .iter()
            .position(|&end| end >= 768)
            .unwrap_or(0)
    } else {
        0
    }
}

fn pad_spectral_unit(encoding: &mut SpectrumEncoding, min_subband_count: usize) {
    while encoding.spectral_unit.subbands.len() < min_subband_count {
        encoding.spectral_unit.subbands.push(SpectralSubband {
            table_index: 0,
            scale_factor_index: None,
            payload: RawBitPayload::default(),
        });
    }
}

fn encode_channel(
    sound_unit: ChannelSoundUnit,
    spectrum: SpectrumEncoding,
) -> Result<PrototypeFrameChannel> {
    let mut writer = BitWriter::new();
    sound_unit.write_to(&mut writer)?;
    let bit_len = writer.bit_len();
    writer.byte_align_zero();

    Ok(PrototypeFrameChannel {
        sound_unit,
        spectrum,
        bit_len,
        bytes: writer.into_bytes(),
    })
}

fn slice_frame(channel: &[f32], start: usize) -> Vec<f32> {
    let mut frame = vec![0.0f32; SAMPLES_PER_FRAME];
    let shifted_start = start as isize + analysis_sample_offset();
    for (dst_index, sample) in frame.iter_mut().enumerate() {
        let src_index = shifted_start + dst_index as isize;
        if (0..channel.len() as isize).contains(&src_index) {
            *sample = channel[src_index as usize];
        }
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::{PrototypeEncoder, PrototypeOptions};
    use crate::{
        atrac3::{
            SAMPLES_PER_FRAME,
            sound_unit::{CodingMode, SpectralTableKind},
        },
        metrics::WavData,
    };

    #[test]
    fn encodes_zero_wav_into_raw_frame() {
        let wav = WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; SAMPLES_PER_FRAME],
        };

        let result = PrototypeEncoder::encode_wav(
            &wav,
            PrototypeOptions {
                coding_mode: CodingMode::Clc,
                lambda: 0.0,
                frame_limit: Some(1),
                start_frame: 0,
                flush_frames: 0,
                target_bits_per_channel: None,
            },
        )
        .unwrap();

        assert_eq!(result.frame_count, 1);
        assert_eq!(result.frames.len(), 1);
        assert_eq!(result.frames[0].channels.len(), 1);
        assert!(!result.frames[0].bytes.is_empty());
        assert_eq!(
            result.frames[0].channels[0].sound_unit.spectrum.subbands[0].table_kind(),
            SpectralTableKind::Skip
        );
    }

    #[test]
    fn packs_stereo_sound_units_without_inter_channel_padding() {
        let wav = WavData {
            sample_rate: 44_100,
            channels: 2,
            samples: vec![0.0; SAMPLES_PER_FRAME * 2],
        };

        let result = PrototypeEncoder::encode_wav(
            &wav,
            PrototypeOptions {
                coding_mode: CodingMode::Clc,
                lambda: 0.0,
                frame_limit: Some(1),
                start_frame: 0,
                flush_frames: 0,
                target_bits_per_channel: None,
            },
        )
        .unwrap();

        let frame = &result.frames[0];
        assert_eq!(frame.channels[0].bit_len, 25);
        assert_eq!(frame.channels[1].bit_len, 25);
        assert_eq!(frame.channels[0].bytes.len(), 4);
        assert_eq!(frame.channels[1].bytes.len(), 4);
        assert_eq!(frame.bit_len, 50);
        assert_eq!(frame.bytes.len(), 7);
    }

    #[test]
    fn pads_partial_tail_frame_with_zeros() {
        let wav = WavData {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.0; SAMPLES_PER_FRAME + 32],
        };

        let result = PrototypeEncoder::encode_wav(
            &wav,
            PrototypeOptions {
                coding_mode: CodingMode::Clc,
                lambda: 0.0,
                frame_limit: None,
                start_frame: 0,
                flush_frames: 0,
                target_bits_per_channel: None,
            },
        )
        .unwrap();

        assert_eq!(result.frame_count, 2);
    }
}
// TEMP DEBUG
