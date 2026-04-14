use anyhow::{Result, ensure};

#[derive(Debug, Clone, Default)]
pub struct BitWriter {
    bytes: Vec<u8>,
    bit_len: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(bytes),
            bit_len: 0,
        }
    }

    pub fn bit_len(&self) -> usize {
        self.bit_len
    }

    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    pub fn write_bit(&mut self, bit: bool) {
        let bit_offset = self.bit_len & 7;
        if bit_offset == 0 {
            self.bytes.push(0);
        }

        if bit {
            let byte_index = self.bytes.len() - 1;
            self.bytes[byte_index] |= 1 << (7 - bit_offset);
        }

        self.bit_len += 1;
    }

    pub fn write_bits(&mut self, value: u32, bits: u8) -> Result<()> {
        ensure!(bits <= 32, "bit width {} exceeds 32", bits);
        if bits == 0 {
            return Ok(());
        }

        let masked = if bits == 32 {
            value
        } else {
            value & ((1u32 << bits) - 1)
        };

        for shift in (0..bits).rev() {
            self.write_bit(((masked >> shift) & 1) != 0);
        }
        Ok(())
    }

    pub fn write_signed(&mut self, value: i32, bits: u8) -> Result<()> {
        ensure!(
            bits > 0 && bits <= 32,
            "signed bit width {} out of range",
            bits
        );
        let encoded = if bits == 32 {
            value as u32
        } else {
            let mask = (1i64 << bits) - 1;
            (value as i64 & mask) as u32
        };
        self.write_bits(encoded, bits)
    }

    pub fn byte_align_zero(&mut self) {
        while (self.bit_len & 7) != 0 {
            self.write_bit(false);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BitReader<'a> {
    bytes: &'a [u8],
    bit_len: usize,
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            bit_len: bytes.len() * 8,
            bit_pos: 0,
        }
    }

    pub fn bit_pos(&self) -> usize {
        self.bit_pos
    }

    pub fn bits_remaining(&self) -> usize {
        self.bit_len.saturating_sub(self.bit_pos)
    }

    pub fn read_bit(&mut self) -> Result<bool> {
        ensure!(self.bit_pos < self.bit_len, "unexpected end of bitstream");
        let byte = self.bytes[self.bit_pos >> 3];
        let bit = (byte >> (7 - (self.bit_pos & 7))) & 1;
        self.bit_pos += 1;
        Ok(bit != 0)
    }

    pub fn read_bits(&mut self, bits: u8) -> Result<u32> {
        ensure!(bits <= 32, "bit width {} exceeds 32", bits);
        ensure!(
            self.bits_remaining() >= bits as usize,
            "requested {} bits with only {} remaining",
            bits,
            self.bits_remaining()
        );

        let mut value = 0u32;
        for _ in 0..bits {
            value = (value << 1) | (self.read_bit()? as u32);
        }
        Ok(value)
    }

    pub fn skip_bits(&mut self, bits: usize) -> Result<()> {
        ensure!(
            self.bits_remaining() >= bits,
            "requested skip of {} bits with only {} remaining",
            bits,
            self.bits_remaining()
        );
        self.bit_pos += bits;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{BitReader, BitWriter};

    #[test]
    fn writes_msb_first() {
        let mut writer = BitWriter::new();
        writer.write_bits(0b101, 3).unwrap();
        writer.write_bits(0b11, 2).unwrap();
        writer.byte_align_zero();

        assert_eq!(writer.bit_len(), 8);
        assert_eq!(writer.as_bytes(), &[0b1011_1000]);
    }

    #[test]
    fn writes_signed_twos_complement() {
        let mut writer = BitWriter::new();
        writer.write_signed(-1, 4).unwrap();
        writer.write_signed(-4, 4).unwrap();
        writer.byte_align_zero();

        assert_eq!(writer.as_bytes(), &[0b1111_1100]);
    }

    #[test]
    fn reads_msb_first() {
        let bytes = [0b1011_1000, 0b0110_0000];
        let mut reader = BitReader::new(&bytes);
        assert_eq!(reader.read_bits(3).unwrap(), 0b101);
        assert_eq!(reader.read_bits(5).unwrap(), 0b11000);
        assert_eq!(reader.read_bits(4).unwrap(), 0b0110);
    }
}
