//! Independent metadata-only Short Text ADD over unchanged historical records.
use super::{SourceCase, source};
use crate::fixtures::access::{canonical_initial_table_creation::inserts::long_rows, put32};

/// Existing nullable Number records precede the new variable declaration.
/// Physical DATA/INDEX bytes and original generation counters remain exact.
pub fn pair(name: &str, values: &[Option<i32>]) -> [Vec<u8>; 2] {
    let before = long_rows(&source(SourceCase::Number), values);
    let after = append_text(&before, name);
    [before, after]
}

/// The independent raw declaration/property append also applies to populated
/// sources: unlike creation from values, it never consults or rebuilds records.
pub fn append_text(before: &[u8], name: &str) -> Vec<u8> {
    append_text_declaration(before, name, 255, true)
}

/// Literal alternate capacities/compression, with exactly the same NULL/default policy.
pub fn append_text_declaration(before: &[u8], name: &str, units: u16, compressed: bool) -> Vec<u8> {
    let mut metadata = before.to_vec();
    for offset in [16, 20, 67] {
        put32(&mut metadata, 7 * 4096 + offset, 0);
    }
    let mut after = super::super::append_text_definition(&metadata, name, units, compressed);
    for offset in [16, 20, 67] {
        let at = 7 * 4096 + offset;
        after[at..at + 4].copy_from_slice(&before[at..at + 4]);
    }
    let original_body = super::super::storage::selected_body(before);
    let body = super::super::properties::append_text(&original_body, name);
    super::super::storage::append(before, after, &body)
}
