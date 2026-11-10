//! Independent retained source with room for an external property descriptor.
use super::canonical_retained_add_table::{self as raw, fields};

fn word(bytes: &[u8], offset: usize) -> usize {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize
}

/// Actual contiguous space between the physical catalog directory and records.
pub fn catalog_gap(bytes: &[u8]) -> usize {
    let page = &bytes[4 * 4096..5 * 4096];
    let count = word(page, 12);
    let start = (0..count)
        .map(|slot| word(page, 14 + 2 * slot) & 0x1fff)
        .min()
        .unwrap();
    start - (14 + count * 2)
}

/// Pack independently authored empty tables until a 255-character Description
/// cannot fit inline, while a named external descriptor and directory cell fit.
/// Live MSysDb metadata, original user rows and all property history remain intact.
pub fn source() -> Vec<u8> {
    let mut bytes = fields::source(fields::Case::FirstNumber).bytes;
    let mut ordinal = 0;
    while catalog_gap(&bytes) > 400 {
        bytes = raw::append(&bytes, &format!("T{ordinal}"), raw::Target::GeneratedNumber);
        ordinal += 1;
    }
    assert!((150..=400).contains(&catalog_gap(&bytes)));
    bytes
}
