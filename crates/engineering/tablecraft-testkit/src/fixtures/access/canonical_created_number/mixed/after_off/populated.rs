//! Independent metadata-only Number ADD over the canonical existing row family.
use super::{SourceCase, source};
use crate::fixtures::access::{canonical_initial_table_creation::inserts::long_rows, put32};

const PAGE: usize = 4096;
const DEFINITION: usize = 7;
const DATA: usize = 8;
const INDEX: usize = 9;
const COUNTERS: [usize; 3] = [16, 20, 67];

/// Dense generated IDs and nullable old Number payloads precede the schema ADD.
/// Their original two-field records remain byte-exact: the new field is absent
/// from those records, which is distinct from a future omitted default of zero.
pub fn pair(name: &str, values: &[Option<i32>]) -> [Vec<u8>; 2] {
    let before = long_rows(&source(SourceCase::Number), values);
    let after = append_number(&before, name);
    [before, after]
}

/// Apply the existing independently authored empty metadata transformation while
/// retaining actual row/index pages and all generation/cardinality counters.
/// Only the canonical source metadata is admitted by this raw fixture helper.
pub fn append_number(before: &[u8], name: &str) -> Vec<u8> {
    let empty = source(SourceCase::Number);
    assert_eq!(before.len(), empty.len());
    let mut metadata = before.to_vec();
    for offset in COUNTERS {
        put32(&mut metadata, DEFINITION * PAGE + offset, 0);
    }
    metadata[DATA * PAGE..(INDEX + 1) * PAGE]
        .copy_from_slice(&empty[DATA * PAGE..(INDEX + 1) * PAGE]);
    assert_eq!(metadata, empty);

    let mut after = super::append_number(&empty, name);
    for offset in COUNTERS {
        let at = DEFINITION * PAGE + offset;
        after[at..at + 4].copy_from_slice(&before[at..at + 4]);
    }
    after[DATA * PAGE..(INDEX + 1) * PAGE]
        .copy_from_slice(&before[DATA * PAGE..(INDEX + 1) * PAGE]);
    after
}

#[cfg(all(test, feature = "access-support"))]
mod tests;
