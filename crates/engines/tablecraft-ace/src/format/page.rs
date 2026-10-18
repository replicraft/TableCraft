//! Jet 4 / initial ACE page geometry; references are in NATIVE_READING.md.
pub(crate) const PAGE_BYTES: usize = 4096;
pub(crate) const JET3_PAGE_BYTES: usize = 2048;
pub(crate) const DATA_SIGNATURE: [u8; 2] = [1, 1];
pub(crate) const TABLE_DEFINITION_SIGNATURE: [u8; 2] = [2, 1];
pub(crate) const USAGE_SIGNATURE: [u8; 4] = [5, 1, 0, 0];
pub(crate) const OWNER_OFFSET: usize = 4;
// Table IDs store a 24-bit definition page; packed references add an 8-bit slot.
pub(crate) const PAGE_NUMBER_MASK: u32 = u32::MAX >> u8::BITS;
