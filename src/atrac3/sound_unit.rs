use anyhow::{Result, ensure};

use super::bitstream::BitWriter;

pub const SOUND_UNIT_ID: u8 = 0x28;
pub const MAX_CODED_QMF_BANDS: usize = 4;
pub const MAX_CODED_SUBBANDS: usize = 32;
pub const MAX_TONAL_COMPONENTS: usize = 64;
pub const TONAL_CELLS_PER_QMF_BAND: usize = 4;
pub const MAX_GAIN_POINTS: usize = 7;
pub const MAX_TONAL_ENTRIES_PER_CELL: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodingMode {
    Vlc = 0,
    Clc = 1,
}

impl CodingMode {
    fn bit(self) -> u32 {
        self as u32
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectralTableKind {
    Skip,
    Pairwise,
    Single,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TonalCodingModeSelector {
    AllVlc = 0,
    AllClc = 1,
    PerComponent = 3,
}

impl TonalCodingModeSelector {
    fn bits(self) -> u32 {
        self as u32
    }

    fn shared_mode(self) -> Option<CodingMode> {
        match self {
            Self::AllVlc => Some(CodingMode::Vlc),
            Self::AllClc => Some(CodingMode::Clc),
            Self::PerComponent => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GainPoint {
    pub level: u8,
    pub location: u8,
}

impl GainPoint {
    fn validate(self) -> Result<()> {
        ensure!(
            self.level <= 0x0f,
            "gain level {} exceeds 4 bits",
            self.level
        );
        ensure!(
            self.location <= 0x1f,
            "gain location {} exceeds 5 bits",
            self.location
        );
        Ok(())
    }

    fn write_to(self, writer: &mut BitWriter) -> Result<()> {
        self.validate()?;
        writer.write_bits(self.level as u32, 4)?;
        writer.write_bits(self.location as u32, 5)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GainBand {
    pub points: Vec<GainPoint>,
}

impl GainBand {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.points.len() <= MAX_GAIN_POINTS,
            "gain band has {} points, max {}",
            self.points.len(),
            MAX_GAIN_POINTS
        );
        for point in &self.points {
            point.validate()?;
        }
        Ok(())
    }

    fn write_to(&self, writer: &mut BitWriter) -> Result<()> {
        self.validate()?;
        writer.write_bits(self.points.len() as u32, 3)?;
        for point in &self.points {
            point.write_to(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitChunk {
    pub value: u32,
    pub bits: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawBitPayload {
    pub chunks: Vec<BitChunk>,
}

impl RawBitPayload {
    pub fn push_bits(&mut self, value: u32, bits: u8) -> Result<()> {
        ensure!(bits <= 32, "payload chunk width {} exceeds 32", bits);
        self.chunks.push(BitChunk { value, bits });
        Ok(())
    }

    pub fn bit_len(&self) -> usize {
        self.chunks.iter().map(|chunk| chunk.bits as usize).sum()
    }

    fn write_to(&self, writer: &mut BitWriter) -> Result<()> {
        for chunk in &self.chunks {
            writer.write_bits(chunk.value, chunk.bits)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TonalEntry {
    pub scale_factor_index: u8,
    pub position: u8,
    pub payload: RawBitPayload,
}

impl TonalEntry {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.scale_factor_index <= 0x3f,
            "tonal sf_index {} exceeds 6 bits",
            self.scale_factor_index
        );
        ensure!(
            self.position <= 0x3f,
            "tonal position {} exceeds 6 bits",
            self.position
        );
        Ok(())
    }

    fn write_to(&self, writer: &mut BitWriter) -> Result<()> {
        self.validate()?;
        writer.write_bits(self.scale_factor_index as u32, 6)?;
        writer.write_bits(self.position as u32, 6)?;
        self.payload.write_to(writer)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TonalCell {
    pub entries: Vec<TonalEntry>,
}

impl TonalCell {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.entries.len() <= MAX_TONAL_ENTRIES_PER_CELL,
            "tonal cell has {} entries, max {}",
            self.entries.len(),
            MAX_TONAL_ENTRIES_PER_CELL
        );
        for entry in &self.entries {
            entry.validate()?;
        }
        Ok(())
    }

    fn write_to(&self, writer: &mut BitWriter) -> Result<()> {
        self.validate()?;
        writer.write_bits(self.entries.len() as u32, 3)?;
        for entry in &self.entries {
            entry.write_to(writer)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TonalComponent {
    pub band_flags: Vec<bool>,
    pub coded_values_minus_one: u8,
    pub quant_step_index: u8,
    pub coding_mode: Option<CodingMode>,
    pub cells: Vec<TonalCell>,
}

impl TonalComponent {
    fn validate(&self, coded_qmf_bands: usize, selector: TonalCodingModeSelector) -> Result<()> {
        ensure!(
            self.band_flags.len() == coded_qmf_bands,
            "tonal component has {} band flags, expected {}",
            self.band_flags.len(),
            coded_qmf_bands
        );
        ensure!(
            self.cells.len() == coded_qmf_bands * TONAL_CELLS_PER_QMF_BAND,
            "tonal component has {} cells, expected {}",
            self.cells.len(),
            coded_qmf_bands * TONAL_CELLS_PER_QMF_BAND
        );
        ensure!(
            self.coded_values_minus_one <= 7,
            "coded_values_minus_one {} exceeds 3 bits",
            self.coded_values_minus_one
        );
        ensure!(
            (2..=7).contains(&self.quant_step_index),
            "quant_step_index {} is outside ATRAC3 encoder range 2..=7",
            self.quant_step_index
        );

        match selector.shared_mode() {
            Some(shared) => {
                if let Some(mode) = self.coding_mode {
                    ensure!(
                        mode == shared,
                        "tonal component mode {:?} conflicts with selector {:?}",
                        mode,
                        selector
                    );
                }
            }
            None => ensure!(
                self.coding_mode.is_some(),
                "per-component tonal mode requires coding_mode to be set"
            ),
        }

        for cell in &self.cells {
            cell.validate()?;
        }
        Ok(())
    }

    pub fn write_to(
        &self,
        writer: &mut BitWriter,
        coded_qmf_bands: usize,
        selector: TonalCodingModeSelector,
    ) -> Result<()> {
        self.validate(coded_qmf_bands, selector)?;

        for flag in &self.band_flags {
            writer.write_bit(*flag);
        }

        writer.write_bits(self.coded_values_minus_one as u32, 3)?;
        writer.write_bits(self.quant_step_index as u32, 3)?;

        if selector == TonalCodingModeSelector::PerComponent {
            writer.write_bits(self.coding_mode.unwrap().bit(), 1)?;
        }

        for (cell_index, cell) in self.cells.iter().enumerate() {
            if !self.band_flags[cell_index / TONAL_CELLS_PER_QMF_BAND] {
                continue;
            }
            cell.write_to(writer)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpectralSubband {
    pub table_index: u8,
    pub scale_factor_index: Option<u8>,
    pub payload: RawBitPayload,
}

impl SpectralSubband {
    pub fn table_kind(&self) -> SpectralTableKind {
        match self.table_index {
            0 => SpectralTableKind::Skip,
            1 => SpectralTableKind::Pairwise,
            _ => SpectralTableKind::Single,
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.table_index <= 7,
            "spectral table index {} exceeds 3 bits",
            self.table_index
        );
        match self.table_index {
            0 => ensure!(
                self.scale_factor_index.is_none(),
                "skipped subband must not carry a scale factor"
            ),
            1 => ensure!(
                self.scale_factor_index.is_some_and(|value| value <= 0x3f),
                "pairwise coded subband must carry a 6-bit scale factor"
            ),
            _ => ensure!(
                self.scale_factor_index.is_some_and(|value| value <= 0x3f),
                "coded subband must carry a 6-bit scale factor"
            ),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpectralUnit {
    pub coding_mode: CodingMode,
    pub subbands: Vec<SpectralSubband>,
}

impl Default for SpectralUnit {
    fn default() -> Self {
        Self {
            coding_mode: CodingMode::Vlc,
            subbands: Vec::new(),
        }
    }
}

impl SpectralUnit {
    fn validate(&self) -> Result<()> {
        ensure!(
            !self.subbands.is_empty() && self.subbands.len() <= MAX_CODED_SUBBANDS,
            "spectral unit has {} coded subbands, expected 1..={}",
            self.subbands.len(),
            MAX_CODED_SUBBANDS
        );
        for subband in &self.subbands {
            subband.validate()?;
        }
        Ok(())
    }

    fn write_to(&self, writer: &mut BitWriter) -> Result<()> {
        self.validate()?;

        writer.write_bits((self.subbands.len() - 1) as u32, 5)?;
        writer.write_bits(self.coding_mode.bit(), 1)?;

        for subband in &self.subbands {
            writer.write_bits(subband.table_index as u32, 3)?;
        }

        for subband in &self.subbands {
            if let Some(scale_factor_index) = subband.scale_factor_index {
                writer.write_bits(scale_factor_index as u32, 6)?;
            }
        }

        for subband in &self.subbands {
            if subband.table_index != 0 {
                subband.payload.write_to(writer)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelSoundUnit {
    pub coded_qmf_bands: u8,
    pub gain_bands: Vec<GainBand>,
    pub tonal_mode_selector: TonalCodingModeSelector,
    pub tonal_components: Vec<TonalComponent>,
    pub spectrum: SpectralUnit,
}

impl Default for ChannelSoundUnit {
    fn default() -> Self {
        Self {
            coded_qmf_bands: 1,
            gain_bands: vec![GainBand::default()],
            tonal_mode_selector: TonalCodingModeSelector::AllVlc,
            tonal_components: Vec::new(),
            spectrum: SpectralUnit::default(),
        }
    }
}

impl ChannelSoundUnit {
    pub fn validate(&self) -> Result<()> {
        let coded_qmf_bands = self.coded_qmf_bands as usize;
        ensure!(
            (1..=MAX_CODED_QMF_BANDS as u8).contains(&self.coded_qmf_bands),
            "coded_qmf_bands {} is outside 1..={}",
            self.coded_qmf_bands,
            MAX_CODED_QMF_BANDS
        );
        ensure!(
            self.gain_bands.len() == coded_qmf_bands,
            "sound unit has {} gain bands, expected {}",
            self.gain_bands.len(),
            coded_qmf_bands
        );
        ensure!(
            self.tonal_components.len() <= MAX_TONAL_COMPONENTS,
            "sound unit has {} tonal components, max {}",
            self.tonal_components.len(),
            MAX_TONAL_COMPONENTS
        );

        for band in &self.gain_bands {
            band.validate()?;
        }

        for component in &self.tonal_components {
            component.validate(coded_qmf_bands, self.tonal_mode_selector)?;
        }

        self.spectrum.validate()?;
        Ok(())
    }

    pub fn write_to(&self, writer: &mut BitWriter) -> Result<()> {
        self.validate()?;

        writer.write_bits(SOUND_UNIT_ID as u32, 6)?;
        writer.write_bits((self.coded_qmf_bands - 1) as u32, 2)?;

        for band in &self.gain_bands {
            band.write_to(writer)?;
        }

        writer.write_bits(self.tonal_components.len() as u32, 5)?;
        if !self.tonal_components.is_empty() {
            writer.write_bits(self.tonal_mode_selector.bits(), 2)?;
            for component in &self.tonal_components {
                component.write_to(
                    writer,
                    self.coded_qmf_bands as usize,
                    self.tonal_mode_selector,
                )?;
            }
        }

        self.spectrum.write_to(writer)?;
        Ok(())
    }

    pub fn bit_len(&self) -> Result<usize> {
        let mut writer = BitWriter::new();
        self.write_to(&mut writer)?;
        Ok(writer.bit_len())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChannelSoundUnit, CodingMode, GainBand, GainPoint, RawBitPayload, SpectralSubband,
        SpectralTableKind, SpectralUnit, TonalCell, TonalCodingModeSelector, TonalComponent,
        TonalEntry,
    };
    use crate::atrac3::bitstream::BitWriter;

    #[test]
    fn writes_minimal_sound_unit_metadata() {
        let unit = ChannelSoundUnit {
            coded_qmf_bands: 1,
            gain_bands: vec![GainBand {
                points: vec![GainPoint {
                    level: 2,
                    location: 3,
                }],
            }],
            tonal_mode_selector: TonalCodingModeSelector::AllVlc,
            tonal_components: Vec::new(),
            spectrum: SpectralUnit {
                coding_mode: CodingMode::Vlc,
                subbands: vec![SpectralSubband {
                    table_index: 2,
                    scale_factor_index: Some(12),
                    payload: RawBitPayload::default(),
                }],
            },
        };

        let mut writer = BitWriter::new();
        unit.write_to(&mut writer).unwrap();
        writer.byte_align_zero();

        assert_eq!(unit.bit_len().unwrap(), 40);
        assert_eq!(writer.as_bytes(), &[0xa0, 0x24, 0x30, 0x00, 0x8c]);
    }

    #[test]
    fn writes_tonal_component_with_per_component_mode() {
        let mut tonal_payload = RawBitPayload::default();
        tonal_payload.push_bits(0b1011, 4).unwrap();

        let mut spectral_payload = RawBitPayload::default();
        spectral_payload.push_bits(0b10, 2).unwrap();

        let unit = ChannelSoundUnit {
            coded_qmf_bands: 2,
            gain_bands: vec![GainBand::default(), GainBand::default()],
            tonal_mode_selector: TonalCodingModeSelector::PerComponent,
            tonal_components: vec![TonalComponent {
                band_flags: vec![true, false],
                coded_values_minus_one: 2,
                quant_step_index: 5,
                coding_mode: Some(CodingMode::Clc),
                cells: vec![
                    TonalCell {
                        entries: vec![TonalEntry {
                            scale_factor_index: 12,
                            position: 4,
                            payload: tonal_payload,
                        }],
                    },
                    TonalCell::default(),
                    TonalCell::default(),
                    TonalCell::default(),
                    TonalCell::default(),
                    TonalCell::default(),
                    TonalCell::default(),
                    TonalCell::default(),
                ],
            }],
            spectrum: SpectralUnit {
                coding_mode: CodingMode::Clc,
                subbands: vec![SpectralSubband {
                    table_index: 3,
                    scale_factor_index: Some(7),
                    payload: spectral_payload,
                }],
            },
        };

        assert_eq!(unit.bit_len().unwrap(), 75);

        let mut writer = BitWriter::new();
        unit.write_to(&mut writer).unwrap();
        writer.byte_align_zero();

        assert_eq!(
            writer.as_bytes(),
            &[0xa1, 0x00, 0x3c, 0xac, 0x98, 0x25, 0x80, 0x01, 0x63, 0xc0]
        );
    }

    #[test]
    fn rejects_mismatched_gain_band_count() {
        let unit = ChannelSoundUnit {
            coded_qmf_bands: 2,
            gain_bands: vec![GainBand::default()],
            ..ChannelSoundUnit::default()
        };

        let error = unit.validate().unwrap_err().to_string();
        assert!(error.contains("expected 2"));
    }

    #[test]
    fn classifies_spectral_table_kinds() {
        assert_eq!(
            SpectralSubband {
                table_index: 0,
                scale_factor_index: None,
                payload: RawBitPayload::default(),
            }
            .table_kind(),
            SpectralTableKind::Skip
        );
        assert_eq!(
            SpectralSubband {
                table_index: 1,
                scale_factor_index: Some(0),
                payload: RawBitPayload::default(),
            }
            .table_kind(),
            SpectralTableKind::Pairwise
        );
        assert_eq!(
            SpectralSubband {
                table_index: 2,
                scale_factor_index: Some(0),
                payload: RawBitPayload::default(),
            }
            .table_kind(),
            SpectralTableKind::Single
        );
    }
}
