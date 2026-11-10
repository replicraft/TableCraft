//! Closed original-page Text replacement fixtures retain their source geometry.
use crate::fixtures::access::{raw_record_placement, raw_record_update};

const PAGE: usize = 4096;
const DEFINITION: usize = 7 * PAGE;
const DATA_PAGE: u32 = 8;

/// The exact original-family terminal; this does not accept an appended table.
pub fn record(bytes: &[u8], root: usize) -> &[u8] {
    assert_eq!(bytes.len(), 11 * PAGE);
    raw_record_placement::record(bytes, DATA_PAGE, root)
}

/// Original stored Text replacement, without broadening its fixed native roles.
pub fn after(before: &[u8], root: usize, column: usize, value: Option<&str>) -> Vec<u8> {
    assert_eq!(before.len(), 11 * PAGE);
    let field = DEFINITION + 75 + column * 25;
    raw_record_update::text(before, DATA_PAGE, root, field, column, value)
}
