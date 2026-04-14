use anyhow::{Result, ensure};

use super::{
    prototype::PrototypeEncodeResult,
    sound_unit::{ChannelSoundUnit, CodingMode},
};

pub const ATRAC3_WAVE_FORMAT_TAG: u16 = 0x0270;
pub const ATRAC3_FMT_CHUNK_SIZE: u32 = 32;
pub const ATRAC3_FACT_CHUNK_SIZE: u32 = 8;
pub const ATRAC3_WAV_EXTRADATA_SIZE: u16 = 14;
pub const ATRAC3_SAMPLES_PER_FRAME: u32 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Atrac3Bitrate {
    Kbps66,
    Kbps105,
    Kbps132,
}

impl Atrac3Bitrate {
    pub fn kbps(self, channels: u16) -> u32 {
        match (channels, self) {
            (1, Self::Kbps66) => 33,
            (1, Self::Kbps105) => 52,
            (1, Self::Kbps132) => 66,
            (_, Self::Kbps66) => 66,
            (_, Self::Kbps105) => 105,
            (_, Self::Kbps132) => 132,
        }
    }

    pub fn block_align(self, channels: u16) -> u16 {
        let per_channel = match self {
            Self::Kbps66 => 96u16,
            Self::Kbps105 => 152u16,
            Self::Kbps132 => 192u16,
        };
        per_channel * channels
    }

    pub fn frame_factor(self) -> u16 {
        1
    }

    pub fn all() -> [Self; 3] {
        [Self::Kbps66, Self::Kbps105, Self::Kbps132]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Atrac3ContainerOptions {
    pub bitrate: Option<Atrac3Bitrate>,
}

impl Default for Atrac3ContainerOptions {
    fn default() -> Self {
        Self { bitrate: None }
    }
}

#[derive(Debug, Clone)]
pub struct Atrac3Container {
    pub bitrate: Atrac3Bitrate,
    pub block_align: u16,
    pub avg_bytes_per_sec: u32,
    pub bytes: Vec<u8>,
}

pub fn wrap_prototype_in_riff_at3(
    encoded: &PrototypeEncodeResult,
    options: Atrac3ContainerOptions,
) -> Result<Atrac3Container> {
    ensure!(
        encoded.sample_rate == 44_100,
        "prototype ATRAC3 container currently expects 44100 Hz, got {}",
        encoded.sample_rate
    );
    ensure!(
        encoded.channel_count >= 1 && encoded.channel_count <= 2,
        "prototype ATRAC3 container currently supports 1 or 2 channels, got {}",
        encoded.channel_count
    );
    ensure!(
        encoded.frame_count > 0,
        "cannot wrap an empty encode result"
    );

    let channels = encoded.channel_count as u16;
    let slot_size_for =
        |bitrate: Atrac3Bitrate| bitrate.block_align(channels) as usize / channels as usize;
    let max_channel_bytes = encoded
        .frames
        .iter()
        .flat_map(|frame| frame.channels.iter())
        .map(|channel| channel.bytes.len())
        .max()
        .unwrap_or(0);
    let bitrate = match options.bitrate {
        Some(bitrate) => {
            ensure!(
                !(channels == 2 && bitrate == Atrac3Bitrate::Kbps66),
                "stereo 66 kbps ATRAC3 requires joint stereo, which is not implemented yet"
            );
            ensure!(
                max_channel_bytes <= slot_size_for(bitrate),
                "encoded channel sound unit requires {} bytes, selected bitrate {} kbps only allows {} bytes per channel",
                max_channel_bytes,
                bitrate.kbps(channels),
                slot_size_for(bitrate)
            );
            bitrate
        }
        None => choose_smallest_fitting_bitrate(max_channel_bytes, channels)?,
    };

    let block_align = bitrate.block_align(channels);
    let channel_slot_size = block_align as usize / channels as usize;
    let avg_bytes_per_sec = ((block_align as u32 * encoded.sample_rate)
        + (ATRAC3_SAMPLES_PER_FRAME / 2))
        / ATRAC3_SAMPLES_PER_FRAME;
    let sample_count = encoded.frame_count as u32 * ATRAC3_SAMPLES_PER_FRAME;
    let coding_mode = coding_mode_extradata(encoded);
    let data_size = encoded.frame_count as u32 * block_align as u32;

    let mut bytes = Vec::with_capacity(76 + data_size as usize);
    let riff_size =
        4 + (8 + ATRAC3_FMT_CHUNK_SIZE) + (8 + ATRAC3_FACT_CHUNK_SIZE) + (8 + data_size);

    bytes.extend_from_slice(b"RIFF");
    push_u32_le(&mut bytes, riff_size);
    bytes.extend_from_slice(b"WAVE");

    bytes.extend_from_slice(b"fmt ");
    push_u32_le(&mut bytes, ATRAC3_FMT_CHUNK_SIZE);
    push_u16_le(&mut bytes, ATRAC3_WAVE_FORMAT_TAG);
    push_u16_le(&mut bytes, channels);
    push_u32_le(&mut bytes, encoded.sample_rate);
    push_u32_le(&mut bytes, avg_bytes_per_sec);
    push_u16_le(&mut bytes, block_align);
    push_u16_le(&mut bytes, 0);
    push_u16_le(&mut bytes, ATRAC3_WAV_EXTRADATA_SIZE);
    push_u16_le(&mut bytes, 1);
    push_u32_le(&mut bytes, 0x1000);
    push_u16_le(&mut bytes, coding_mode);
    push_u16_le(&mut bytes, coding_mode);
    push_u16_le(&mut bytes, bitrate.frame_factor());
    push_u16_le(&mut bytes, 0);

    bytes.extend_from_slice(b"fact");
    push_u32_le(&mut bytes, ATRAC3_FACT_CHUNK_SIZE);
    push_u32_le(&mut bytes, sample_count);
    push_u32_le(&mut bytes, ATRAC3_SAMPLES_PER_FRAME);

    bytes.extend_from_slice(b"data");
    push_u32_le(&mut bytes, data_size);

    for frame in &encoded.frames {
        ensure!(
            frame.channels.len() == encoded.channel_count,
            "frame contains {} channels, expected {}",
            frame.channels.len(),
            encoded.channel_count
        );

        for channel in &frame.channels {
            ensure!(
                channel.bytes.len() <= channel_slot_size,
                "channel sound unit {} exceeds per-channel slot {}",
                channel.bytes.len(),
                channel_slot_size
            );
            bytes.extend_from_slice(&channel.bytes);
            let padding = channel_slot_size - channel.bytes.len();
            if padding > 0 {
                bytes.resize(bytes.len() + padding, 0);
            }
        }
    }

    Ok(Atrac3Container {
        bitrate,
        block_align,
        avg_bytes_per_sec,
        bytes,
    })
}

fn choose_smallest_fitting_bitrate(
    max_channel_bytes: usize,
    channels: u16,
) -> Result<Atrac3Bitrate> {
    Atrac3Bitrate::all()
        .into_iter()
        .find(|bitrate| {
            !(channels == 2 && *bitrate == Atrac3Bitrate::Kbps66)
                && max_channel_bytes <= bitrate.block_align(channels) as usize / channels as usize
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "encoded channel sound unit requires {} bytes, exceeds supported ATRAC3 budgets",
                max_channel_bytes
            )
        })
}

fn coding_mode_extradata(encoded: &PrototypeEncodeResult) -> u16 {
    let has_joint_stereo = encoded
        .frames
        .iter()
        .flat_map(|frame| frame.channels.iter())
        .any(|channel| {
            matches!(channel.sound_unit.spectrum.coding_mode, CodingMode::Vlc)
                && channel.sound_unit.coded_qmf_bands > 1
                && encoded.channel_count == 2
                && has_joint_stereo_marker(&channel.sound_unit)
        });

    if has_joint_stereo { 1 } else { 0 }
}

fn has_joint_stereo_marker(_sound_unit: &ChannelSoundUnit) -> bool {
    false
}

fn push_u16_le(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32_le(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::{
        ATRAC3_FACT_CHUNK_SIZE, ATRAC3_FMT_CHUNK_SIZE, ATRAC3_WAVE_FORMAT_TAG, Atrac3Bitrate,
        Atrac3ContainerOptions, wrap_prototype_in_riff_at3,
    };
    use crate::{
        atrac3::{
            SAMPLES_PER_FRAME,
            prototype::{PrototypeEncoder, PrototypeOptions},
            sound_unit::CodingMode,
        },
        metrics::WavData,
    };

    #[test]
    fn wraps_prototype_frame_in_riff_container() {
        let wav = WavData {
            sample_rate: 44_100,
            channels: 2,
            samples: vec![0.0; SAMPLES_PER_FRAME * 2],
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
        assert_eq!(wrapped.block_align, Atrac3Bitrate::Kbps105.block_align(2));
        assert_eq!(&wrapped.bytes[0..4], b"RIFF");
        assert_eq!(&wrapped.bytes[8..12], b"WAVE");
        assert_eq!(
            u32::from_le_bytes(wrapped.bytes[16..20].try_into().unwrap()),
            ATRAC3_FMT_CHUNK_SIZE
        );
        assert_eq!(
            u16::from_le_bytes(wrapped.bytes[20..22].try_into().unwrap()),
            ATRAC3_WAVE_FORMAT_TAG
        );
        assert_eq!(
            u32::from_le_bytes(wrapped.bytes[56..60].try_into().unwrap()),
            ATRAC3_FACT_CHUNK_SIZE
        );
        assert_eq!(
            u32::from_le_bytes(wrapped.bytes[60..64].try_into().unwrap()),
            SAMPLES_PER_FRAME as u32
        );
        assert_eq!(wrapped.bytes[76], 0xa0);
        assert_eq!(wrapped.bytes[76 + wrapped.block_align as usize / 2], 0xa0);
    }
}
