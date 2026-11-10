//! Independent companion-cell updates in actual appended generated tables.
use super::word;
use crate::fixtures::access::raw_record_update;

const PAGE: usize = 4096;
const SECOND_FIELD: usize = 100;

fn companion(before: &[u8], definition: u32, kind: u8) -> usize {
    let page = definition as usize * PAGE;
    assert_eq!(word(before, page + 41), 2);
    assert_eq!(word(before, page + 45), 2);
    assert_eq!(before[page + 75 + 15], 7);
    let field = page + SECOND_FIELD;
    assert_eq!(word(before, field + 5), 1);
    assert_eq!(before[field], kind);
    field
}

/// Preserve generated ID, stored counters, indexes and NULL's ignored payload.
pub fn number(before: &[u8], definition: u32, root: usize, value: Option<i32>) -> Vec<u8> {
    let field = companion(before, definition, 4);
    raw_record_update::number(before, definition + 1, root, field, 1, value)
}

/// Preserve logical root references and append hidden same-owner targets only
/// when growth requires it; retain prior hidden targets and unrelated padding.
pub fn text(before: &[u8], definition: u32, root: usize, value: Option<&str>) -> Vec<u8> {
    let field = companion(before, definition, 10);
    raw_record_update::text(before, definition + 1, root, field, 1, value)
}

/// Retain a supplied Text payload behind a NULL bit for preservation checks.
pub fn with_opaque_text_null(before: &[u8], definition: u32, root: usize) -> Vec<u8> {
    companion(before, definition, 10);
    let old = crate::fixtures::access::raw_record_placement::record(before, definition + 1, root);
    let mut row = old.to_vec();
    let mask = row.len() - 1;
    assert_eq!(row[mask] & 2, 2);
    row[mask] &= !2;
    crate::fixtures::access::raw_record_placement::place(before, definition + 1, root, &row)
}
