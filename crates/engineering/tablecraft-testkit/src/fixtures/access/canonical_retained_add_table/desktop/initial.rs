//! Exact initial desktop declaration plus its independently authored policy body.
use crate::fixtures::access::{
    canonical_created_default_zero, canonical_creation_properties,
    canonical_initial_table_creation::{self as initial, Column},
    put16,
};

const PAGE: usize = 4096;

fn word(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
}

pub(in crate::fixtures::access::canonical_retained_add_table) fn table(
    columns: &'static [Column],
    properties: &[u8],
) -> Vec<u8> {
    let mut bytes =
        canonical_created_default_zero::source(canonical_created_default_zero::Source::IdOnly);
    let declaration = initial::encode_columns(columns);
    bytes[7 * PAGE..8 * PAGE].copy_from_slice(&declaration.bytes[7 * PAGE..8 * PAGE]);
    super::definition(&mut bytes[7 * PAGE..8 * PAGE]);

    let page = &mut bytes[4 * PAGE..5 * PAGE];
    assert_eq!(word(page, 12), 8);
    let old_start = word(page, 28);
    let end = word(page, 26);
    let record = canonical_creation_properties::attach(&page[old_start..end], properties);
    let start = end - record.len();
    page[start..end].copy_from_slice(&record);
    put16(page, 28, start as u16);
    put16(page, 2, (start - 30) as u16);
    bytes
}
