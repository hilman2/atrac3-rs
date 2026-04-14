pub mod bitstream;
pub mod container;
pub mod gain;
pub mod inspect;
pub mod mdct;
pub mod prototype;
pub mod qmf;
pub mod quant;
pub mod sound_unit;
pub mod synthesis;

pub const SAMPLES_PER_FRAME: usize = 1024;
pub const QMF_BANDS: usize = 4;
pub const SAMPLES_PER_BAND: usize = SAMPLES_PER_FRAME / QMF_BANDS;
