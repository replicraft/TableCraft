//! Data-page slot geometry, packed row references and record word widths.
pub(crate) const ROW_COUNT_OFFSET: usize = 12;
const SLOT_ARRAY_OFFSET: usize = 14;
pub(crate) const SLOT_BYTES: usize = size_of::<u16>();
pub(crate) const MAX_SLOTS: usize = u8::MAX as usize;
pub(crate) const OFFSET_MASK: u16 = 0x1fff;
pub(crate) const DELETED_MASK: u16 = 0x8000;
pub(crate) const RELOCATED_MASK: u16 = 0x4000;
pub(crate) const UNSUPPORTED_MASK: u16 = 0x2000;
pub(crate) const COLUMN_COUNT_BYTES: usize = size_of::<u16>();
pub(crate) const VARIABLE_COUNT_BYTES: usize = size_of::<u16>();
pub(crate) const VARIABLE_OFFSET_BYTES: usize = size_of::<u16>();
pub(crate) const MASK_BITS: usize = u8::BITS as usize;
pub(crate) const fn slot_offset(slot: usize) -> usize {
    SLOT_ARRAY_OFFSET + SLOT_BYTES * slot
}
pub(crate) const fn reference_page(reference: u32) -> u32 {
    reference >> u8::BITS
}
pub(crate) const fn reference_slot(reference: u32) -> u8 {
    reference as u8
}
pub(crate) const fn reference(page: u32, slot: u8) -> u32 {
    (page << u8::BITS) | slot as u32
}
