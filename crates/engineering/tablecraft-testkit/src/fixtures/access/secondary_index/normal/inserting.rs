//! Complete manual root-leaf INSERT images after ordinary CREATE and indexed UPDATE.
pub mod compressed;
use super::{Row, updating};
use crate::fixtures::access::{multiple_indexes, packed_record::append_record, put32};
use std::collections::BTreeSet;

const PAGE: usize = 4096;

/// The genuine prior write stages remain explicit inputs to lifecycle tests.
pub struct Case {
    pub updated: updating::Case,
    pub expected: Vec<u8>,
    pub values: Vec<Option<i32>>,
}

/// Missing/explicit NULL, repeated existing keys and a new key are distinct
/// requests even where their final stored nullable carrier is identical.
pub fn source(descending: bool) -> Case {
    let updated = updating::incremented(descending);
    let values = vec![None, None, Some(1), Some(99)];
    let expected = after(&updated, &values);
    Case {
        updated,
        expected,
        values,
    }
}

/// Existing unique-only indexes admit another NULL; Required indexes do not.
/// No CREATE UNIQUE API or generated-value policy is supplied by this fixture.
pub fn unique(descending: bool, required: bool) -> Case {
    let updated = updating::unique(descending, required);
    let values = if required {
        vec![Some(7)]
    } else {
        vec![None, Some(7)]
    };
    let expected = after(&updated, &values);
    Case {
        updated,
        expected,
        values,
    }
}

fn word(bytes: &[u8], at: usize) -> usize {
    u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()) as usize
}
fn long(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// Literal full-image expectation for one manual native-zero LONG and its one
/// local index. It reuses the real packed DATA and existing-page raw frame owners.
pub fn after(updated: &updating::Case, values: &[Option<i32>]) -> Vec<u8> {
    let source = &updated.creation;
    let before = &updated.expected;
    let definition = source.target_definition as usize * PAGE;
    let data = (source.target_definition as usize + 1) * PAGE;
    let root = source.root as usize * PAGE;
    assert_eq!(
        &before[definition + 41..definition + 47],
        &[1, 0, 0, 0, 1, 0]
    );
    assert_eq!(
        &before[definition + 47..definition + 55],
        &[1, 0, 0, 0, 1, 0, 0, 0]
    );
    assert_eq!(&before[definition + 75..definition + 76], &[4]);
    assert_eq!(&before[definition + 80..definition + 82], &[0, 0]);
    assert_eq!(&before[definition + 90..definition + 92], &[3, 0]);
    assert_eq!(&before[definition + 96..definition + 100], &[0, 0, 4, 0]);
    assert_eq!(source.index_pages, [source.root]);
    assert_eq!(&before[root..root + 2], &[4, 1]);
    assert_eq!(&before[root + 8..root + 24], &[0; 16]);
    assert_eq!(word(before, root + 24), 0);
    assert_eq!(
        &before[data + 4..data + 8],
        &source.target_definition.to_le_bytes()
    );
    assert_eq!(word(before, data + 12), source.rows.len());
    let mut rows = source.rows.to_vec();
    for cell in &updated.cells {
        assert_eq!(rows[cell.row].score, cell.expected);
        rows[cell.row].score = cell.value;
    }
    let mut families = rows.iter().map(|row| row.score).collect::<BTreeSet<_>>();
    let old_distinct = families.len();
    let mut result = before.to_vec();
    for value in values {
        let slot = word(&result, data + 12);
        assert!(slot < 255);
        let mut record = vec![1, 0];
        record.extend(value.unwrap_or(0).to_le_bytes());
        record.push(u8::from(value.is_some()));
        append_record(&mut result[data..data + PAGE], &record);
        rows.push(Row {
            score: *value,
            reference: ((source.target_definition + 1) << 8) | slot as u32,
        });
        families.insert(*value);
    }
    put32(
        &mut result,
        definition + 16,
        long(before, definition + 16) + values.len() as u32,
    );
    // The old history can exceed current distinct membership after UPDATE.
    // Add only genuinely new families; never replace history with live count.
    put32(
        &mut result,
        definition + 67,
        long(before, definition + 67) + (families.len() - old_distinct) as u32,
    );
    let mut entries = rows
        .iter()
        .map(|row| {
            let mut key = match row.score {
                Some(value) => {
                    let mut key = vec![127];
                    key.extend(((value as u32) ^ 0x8000_0000).to_be_bytes());
                    key
                }
                None => vec![0],
            };
            if source.descending {
                for byte in &mut key {
                    *byte = !*byte;
                }
            }
            key.extend(row.reference.to_be_bytes());
            key
        })
        .collect::<Vec<_>>();
    entries.sort();
    let page = multiple_indexes::leaf_owned(
        Some(&before[root..root + PAGE]),
        &entries,
        0,
        0,
        0,
        source.target_definition,
    );
    result[root..root + PAGE].copy_from_slice(&page);
    assert_eq!(
        &result[definition + 20..definition + 24],
        &before[definition + 20..definition + 24]
    );
    assert_eq!(result.len(), before.len());
    result
}
