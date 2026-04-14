use anyhow::{Context, Result, bail, ensure};

use super::{
    bitstream::BitReader,
    container::ATRAC3_WAVE_FORMAT_TAG,
    gain::{DecoderWindowKind, decoder_window_kind},
    sound_unit::SOUND_UNIT_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedGainPoint {
    pub level: u8,
    pub location: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedChannelPrefix {
    pub coded_qmf_bands: u8,
    pub gain_bands: Vec<Vec<ParsedGainPoint>>,
    pub tonal_component_count: u8,
    pub tonal_mode_selector: Option<u8>,
    pub consumed_bits: usize,
}

#[derive(Debug, Clone)]
pub struct ParsedAtrac3Container {
    pub channels: u16,
    pub block_align: u16,
    pub data_offset: usize,
    pub data_size: usize,
    pub frame_count: usize,
}

pub fn parse_channel_prefix(bytes: &[u8]) -> Result<ParsedChannelPrefix> {
    let mut reader = BitReader::new(bytes);
    let sound_unit_id = reader.read_bits(6)? as u8;
    ensure!(
        sound_unit_id == SOUND_UNIT_ID,
        "unexpected sound unit id 0x{:02x}, expected 0x{:02x}",
        sound_unit_id,
        SOUND_UNIT_ID
    );

    let coded_qmf_bands = reader.read_bits(2)? as u8 + 1;
    let mut gain_bands = Vec::with_capacity(coded_qmf_bands as usize);
    for _ in 0..coded_qmf_bands {
        let point_count = reader.read_bits(3)? as usize;
        let mut points = Vec::with_capacity(point_count);
        for _ in 0..point_count {
            let level = reader.read_bits(4)? as u8;
            let location = reader.read_bits(5)? as u8;
            points.push(ParsedGainPoint { level, location });
        }
        gain_bands.push(points);
    }

    let tonal_component_count = reader.read_bits(5)? as u8;
    let tonal_mode_selector = if tonal_component_count == 0 {
        None
    } else {
        Some(reader.read_bits(2)? as u8)
    };

    Ok(ParsedChannelPrefix {
        coded_qmf_bands,
        gain_bands,
        tonal_component_count,
        tonal_mode_selector,
        consumed_bits: reader.bit_pos(),
    })
}

pub fn parse_riff_atrac3(bytes: &[u8]) -> Result<ParsedAtrac3Container> {
    ensure!(bytes.len() >= 12, "file too small for RIFF header");
    ensure!(&bytes[0..4] == b"RIFF", "missing RIFF signature");
    ensure!(&bytes[8..12] == b"WAVE", "missing WAVE signature");

    let mut cursor = 12usize;
    let mut channels = None;
    let mut block_align = None;
    let mut data_offset = None;
    let mut data_size = None;

    while cursor + 8 <= bytes.len() {
        let chunk_id = &bytes[cursor..cursor + 4];
        let chunk_size =
            u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let chunk_data_offset = cursor + 8;
        let padded_chunk_size = chunk_size + (chunk_size & 1);
        ensure!(
            chunk_data_offset + padded_chunk_size <= bytes.len(),
            "chunk {:?} overruns file",
            String::from_utf8_lossy(chunk_id)
        );

        if chunk_id == b"fmt " {
            ensure!(chunk_size >= 16, "fmt chunk too small: {}", chunk_size);
            let format_tag = u16::from_le_bytes(
                bytes[chunk_data_offset..chunk_data_offset + 2]
                    .try_into()
                    .unwrap(),
            );
            ensure!(
                format_tag == ATRAC3_WAVE_FORMAT_TAG,
                "unexpected WAVE format tag 0x{:04x}",
                format_tag
            );
            channels = Some(u16::from_le_bytes(
                bytes[chunk_data_offset + 2..chunk_data_offset + 4]
                    .try_into()
                    .unwrap(),
            ));
            block_align = Some(u16::from_le_bytes(
                bytes[chunk_data_offset + 12..chunk_data_offset + 14]
                    .try_into()
                    .unwrap(),
            ));
        } else if chunk_id == b"data" {
            data_offset = Some(chunk_data_offset);
            data_size = Some(chunk_size);
        }

        cursor = chunk_data_offset + padded_chunk_size;
    }

    let channels = channels.context("missing fmt chunk")?;
    let block_align = block_align.context("missing block_align in fmt chunk")?;
    let data_offset = data_offset.context("missing data chunk")?;
    let data_size = data_size.context("missing data size")?;
    ensure!(channels > 0, "invalid channel count {}", channels);
    ensure!(block_align > 0, "invalid block_align {}", block_align);
    ensure!(
        data_size % block_align as usize == 0,
        "data chunk {} is not aligned to block_align {}",
        data_size,
        block_align
    );

    Ok(ParsedAtrac3Container {
        channels,
        block_align,
        data_offset,
        data_size,
        frame_count: data_size / block_align as usize,
    })
}

pub fn extract_channel_slot<'a>(
    bytes: &'a [u8],
    container: &ParsedAtrac3Container,
    frame_index: usize,
    channel_index: usize,
) -> Result<&'a [u8]> {
    ensure!(
        frame_index < container.frame_count,
        "frame {} out of range 0..{}",
        frame_index,
        container.frame_count
    );
    ensure!(
        channel_index < container.channels as usize,
        "channel {} out of range 0..{}",
        channel_index,
        container.channels
    );

    let channel_slot_size = container.block_align as usize / container.channels as usize;
    let frame_offset = container.data_offset + frame_index * container.block_align as usize;
    let slot_offset = frame_offset + channel_index * channel_slot_size;
    Ok(&bytes[slot_offset..slot_offset + channel_slot_size])
}

pub fn band_window_kinds(
    previous: Option<&ParsedChannelPrefix>,
    current: &ParsedChannelPrefix,
) -> Vec<DecoderWindowKind> {
    let previous_bands = previous.map(|prefix| &prefix.gain_bands);
    current
        .gain_bands
        .iter()
        .enumerate()
        .map(|(band_index, current_band)| {
            let previous_active = previous_bands
                .and_then(|bands| bands.get(band_index))
                .is_some_and(|band| !band.is_empty());
            let current_active = !current_band.is_empty();
            decoder_window_kind(previous_active, current_active)
        })
        .collect()
}

pub fn format_window_kind(kind: DecoderWindowKind) -> &'static str {
    match kind {
        DecoderWindowKind::Full => "full",
        DecoderWindowKind::ZeroTail => "zero_tail",
        DecoderWindowKind::ZeroHead => "zero_head",
        DecoderWindowKind::ZeroEdges => "zero_edges",
    }
}

pub fn summarize_gain_activity(prefixes: &[ParsedChannelPrefix]) -> Vec<(usize, usize)> {
    let max_bands = prefixes
        .iter()
        .map(|prefix| prefix.gain_bands.len())
        .max()
        .unwrap_or(0);
    let mut summary = vec![(0usize, 0usize); max_bands];

    for prefix in prefixes {
        for (band_index, points) in prefix.gain_bands.iter().enumerate() {
            if !points.is_empty() {
                summary[band_index].0 += 1;
                summary[band_index].1 += points.len();
            }
        }
    }

    summary
}

pub fn parse_prefixes_for_channel(
    bytes: &[u8],
    container: &ParsedAtrac3Container,
    channel_index: usize,
) -> Result<Vec<ParsedChannelPrefix>> {
    let mut prefixes = Vec::with_capacity(container.frame_count);
    for frame_index in 0..container.frame_count {
        let slot = extract_channel_slot(bytes, container, frame_index, channel_index)?;
        prefixes.push(parse_channel_prefix(slot).with_context(|| {
            format!(
                "failed to parse frame {} channel {}",
                frame_index, channel_index
            )
        })?);
    }
    Ok(prefixes)
}

pub fn ensure_channel_slots(container: &ParsedAtrac3Container) -> Result<()> {
    if container.block_align as usize % container.channels as usize != 0 {
        bail!(
            "block_align {} is not divisible by channels {}",
            container.block_align,
            container.channels
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ParsedGainPoint, band_window_kinds, format_window_kind, parse_channel_prefix,
        parse_riff_atrac3,
    };
    use crate::atrac3::{
        container::{Atrac3ContainerOptions, wrap_prototype_in_riff_at3},
        prototype::{PrototypeEncoder, PrototypeOptions},
        sound_unit::CodingMode,
    };
    use crate::metrics::WavData;

    #[test]
    fn parses_gain_prefix_from_minimal_sound_unit() {
        let bytes = [0b1010_0000, 0b0000_0000];
        let prefix = parse_channel_prefix(&bytes).unwrap();
        assert_eq!(prefix.coded_qmf_bands, 1);
        assert!(prefix.gain_bands[0].is_empty());
        assert_eq!(prefix.tonal_component_count, 0);
    }

    #[test]
    fn derives_window_kind_from_gain_activity() {
        let previous = crate::atrac3::inspect::ParsedChannelPrefix {
            coded_qmf_bands: 1,
            gain_bands: vec![vec![ParsedGainPoint {
                level: 3,
                location: 7,
            }]],
            tonal_component_count: 0,
            tonal_mode_selector: None,
            consumed_bits: 0,
        };
        let current = crate::atrac3::inspect::ParsedChannelPrefix {
            coded_qmf_bands: 1,
            gain_bands: vec![vec![]],
            tonal_component_count: 0,
            tonal_mode_selector: None,
            consumed_bits: 0,
        };
        let kinds = band_window_kinds(Some(&previous), &current);
        assert_eq!(format_window_kind(kinds[0]), "zero_head");
    }

    #[test]
    fn parses_generated_riff_container() {
        let wav = WavData {
            sample_rate: 44_100,
            channels: 2,
            samples: vec![0.0; 1024 * 2],
        };
        let encoded = PrototypeEncoder::encode_wav(
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
        let wrapped =
            wrap_prototype_in_riff_at3(&encoded, Atrac3ContainerOptions::default()).unwrap();
        let parsed = parse_riff_atrac3(&wrapped.bytes).unwrap();
        assert_eq!(parsed.channels, 2);
        assert_eq!(parsed.frame_count, 1);
    }
}
