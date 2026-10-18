//! Little-endian word scanning, selected by the bitmap kernel benchmark.
pub(crate) const BLOCK_BYTES: usize = size_of::<u64>();

/// A caller checks/charges a bounded block before enumerating its set bits.
#[inline]
pub(crate) fn set_bits(bytes: &[u8]) -> SetBits {
    assert!(
        bytes.len() <= BLOCK_BYTES,
        "bitmap block exceeds word width"
    );
    let mut word = [0; BLOCK_BYTES];
    word[..bytes.len()].copy_from_slice(bytes);
    SetBits(u64::from_le_bytes(word))
}

pub(crate) struct SetBits(u64);

impl Iterator for SetBits {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros() as usize;
        self.0 &= self.0 - 1;
        Some(bit)
    }
}
