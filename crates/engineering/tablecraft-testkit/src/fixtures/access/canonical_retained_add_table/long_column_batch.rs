//! Independent atomic LONG replacements over original source ordinals.
use super::{
    field_changes, fields, joined_update, populated, typed_fields::Kind, unadorned_fields,
};
use crate::fixtures::access::raw_forwarding_chain;
use tablecraft_core::values::Value;

const PAGE: usize = 4096;

/// Exact original and replacement carriers, without native update codecs.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub row: usize,
    pub expected: Option<i32>,
    pub value: Option<i32>,
}

/// Complete source and independent replacement with every companion byte retained.
pub struct Source {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub table: &'static str,
    pub column: &'static str,
    pub definition: u32,
    pub cells: Vec<Cell>,
}

/// Existing primary, Other and Changes remain intact while both Score rows change.
pub fn two_rows() -> Source {
    let source = joined_update::source();
    finish_score(
        source.bytes,
        source.target_definition,
        vec![
            Cell {
                row: 0,
                expected: Some(17),
                value: Some(22),
            },
            Cell {
                row: 1,
                expected: None,
                value: Some(5),
            },
        ],
    )
}

/// A three-request batch includes NULL preservation and an unchanged request.
pub fn three_rows() -> Source {
    let source = joined_update::source();
    let before = field_changes::append_score(&source.bytes, source.target_definition, Some(19));
    finish_score(
        before,
        source.target_definition,
        vec![
            Cell {
                row: 0,
                expected: Some(17),
                value: None,
            },
            Cell {
                row: 1,
                expected: None,
                value: None,
            },
            Cell {
                row: 2,
                expected: Some(19),
                value: Some(i32::MIN),
            },
        ],
    )
}

/// The original logical ordinal still identifies its forwarded terminal record.
pub fn forwarded() -> Source {
    let source = joined_update::source();
    let before = raw_forwarding_chain::extend(&source.bytes, source.target_definition + 1, 0, 20);
    finish_score(
        before,
        source.target_definition,
        vec![
            Cell {
                row: 0,
                expected: Some(17),
                value: Some(22),
            },
            Cell {
                row: 1,
                expected: None,
                value: Some(5),
            },
        ],
    )
}

fn finish_score(before: Vec<u8>, definition: u32, cells: Vec<Cell>) -> Source {
    let mut expected = before.to_vec();
    for cell in &cells {
        let old = raw_forwarding_chain::record(&before, definition + 1, cell.row);
        assert_eq!(&old[..2], &[2, 0]);
        let present = old[old.len() - 1] & 2 != 0;
        let value = present.then(|| i32::from_le_bytes(old[6..10].try_into().unwrap()));
        assert_eq!(value, cell.expected);
        expected = field_changes::update_score(&expected, definition, cell.row, cell.value);
    }
    Source {
        before,
        expected,
        table: "Table1",
        column: "Score",
        definition,
        cells,
    }
}

/// Literal manual native0 LONG spans the actual 255-slot DATA page boundary.
/// This is a finite test population, not an admission or operation row ceiling.
pub fn cross_page() -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / PAGE) as u32;
    let values = (0..260)
        .map(|row| (row % 17 != 0).then_some(row))
        .collect::<Vec<_>>();
    let before = populated::numbers(&seed, "BatchValues", &values);
    let base = definition as usize * PAGE;
    assert_eq!(&before[base + 41..base + 47], &[1, 0, 0, 0, 1, 0]);
    assert_eq!(&before[base + 47..base + 55], &[0; 8]);
    assert_eq!(before[base + 63], 4);
    assert_eq!(&before[base + 68..base + 70], &[0, 0]);
    assert_eq!(&before[base + 78..base + 80], &[3, 0]);
    assert_eq!(&before[base + 84..base + 88], &[0, 0, 4, 0]);
    let cells = vec![
        Cell {
            row: 254,
            expected: Some(254),
            value: None,
        },
        Cell {
            row: 255,
            expected: None,
            value: Some(i32::MIN),
        },
        Cell {
            row: 256,
            expected: Some(256),
            value: Some(i32::MAX),
        },
        Cell {
            row: 259,
            expected: Some(259),
            value: Some(259),
        },
    ];
    let mut references = Vec::new();
    let mut number = definition + 1;
    while references.len() < values.len() {
        let page = number as usize * PAGE;
        assert_eq!(&before[page..page + 2], &[1, 1]);
        assert_eq!(&before[page + 4..page + 8], &definition.to_le_bytes());
        let count = u16::from_le_bytes(before[page + 12..page + 14].try_into().unwrap());
        for slot in 0..count {
            references.push((number, usize::from(slot)));
        }
        number += 1;
    }
    assert_eq!(references.len(), 260);
    assert_eq!(references[254], (definition + 1, 254));
    assert_eq!(references[255], (definition + 2, 0));
    assert_eq!(references[256], (definition + 2, 1));
    let mut expected = before.to_vec();
    for cell in &cells {
        let (page, slot) = references[cell.row];
        let old = raw_forwarding_chain::record(&before, page, slot);
        assert_eq!(old.len(), 7);
        assert_eq!(&old[..2], &[1, 0]);
        let value = (old[6] & 1 != 0).then(|| i32::from_le_bytes(old[2..6].try_into().unwrap()));
        assert_eq!(value, cell.expected);
        let mut row = old.to_vec();
        if let Some(value) = cell.value {
            row[2..6].copy_from_slice(&value.to_le_bytes());
            row[6] |= 1;
        } else {
            row[6] &= !1;
        }
        expected = raw_forwarding_chain::replace_terminal(&expected, page, slot, &row);
    }
    Source {
        before,
        expected,
        table: "BatchValues",
        column: "CopiedScore",
        definition,
        cells,
    }
}

/// Both original rows lack the newly declared LONG. Materializing them reserves
/// two distinct hidden records while preserving the old Score/primary history.
pub fn materializing() -> Source {
    let source = joined_update::source();
    let definition = source.target_definition;
    let before =
        unadorned_fields::after(&source.bytes, definition, "ExtraScore", Kind::Int32, [0; 4]);
    let cells = vec![
        Cell {
            row: 0,
            expected: None,
            value: Some(22),
        },
        Cell {
            row: 1,
            expected: None,
            value: Some(5),
        },
    ];
    let mut expected = before.to_vec();
    for cell in &cells {
        let old = raw_forwarding_chain::record(&before, definition + 1, cell.row);
        assert_eq!(old.len(), 11);
        assert_eq!(&old[..2], &[2, 0]);
        expected = unadorned_fields::rows::update(
            &expected,
            definition,
            cell.row,
            Kind::Int32,
            &Value::Int32(cell.value.unwrap()),
        );
    }
    for cell in &cells {
        let next = raw_forwarding_chain::record(&expected, definition + 1, cell.row);
        assert_eq!(next.len(), 15);
        assert_eq!(&next[..2], &[3, 0]);
    }
    let base = (definition as usize + 1) * PAGE;
    assert_eq!(&before[base + 12..base + 14], &[2, 0]);
    assert_eq!(&expected[base + 12..base + 14], &[4, 0]);
    Source {
        before,
        expected,
        table: "Table1",
        column: "ExtraScore",
        definition,
        cells,
    }
}
