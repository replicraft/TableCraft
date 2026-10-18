//! Bounded byte-SIMD selection followed by scalar set-bit enumeration.
use wide::u8x16;

pub(crate) const BLOCK_BYTES: usize = size_of::<u8x16>();
const BITS_PER_BYTE: usize = u8::BITS as usize;

/// Visit chunks of at most BLOCK_BYTES after charging/checking their work.
pub(crate) fn set_bits(bytes: &[u8]) -> SetBits<'_> {
    assert!(
        bytes.len() <= BLOCK_BYTES,
        "bitmap block exceeds SIMD width"
    );
    let nonzero = if let Ok(array) = <&[u8; BLOCK_BYTES]>::try_from(bytes) {
        // A byte vector accepts every bit pattern. Casting the array by value
        // needs no pointer alignment and has no integer-endianness interpretation.
        let lanes: u8x16 = bytemuck::cast(*array);
        lanes.simd_ne(u8x16::splat(0)).to_bitmask() as u16
    } else {
        bytes
            .iter()
            .enumerate()
            .fold(0, |mask, (i, byte)| mask | (u16::from(*byte != 0) << i))
    };
    SetBits {
        bytes,
        nonzero,
        current: 0,
        byte_index: 0,
    }
}

pub(crate) struct SetBits<'a> {
    bytes: &'a [u8],
    nonzero: u16,
    current: u8,
    byte_index: usize,
}

impl Iterator for SetBits<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == 0 {
            if self.nonzero == 0 {
                return None;
            }
            self.byte_index = self.nonzero.trailing_zeros() as usize;
            self.nonzero &= self.nonzero - 1;
            self.current = self.bytes[self.byte_index];
        }
        let bit = self.current.trailing_zeros() as usize;
        self.current &= self.current - 1;
        Some(self.byte_index * BITS_PER_BYTE + bit)
    }
}
