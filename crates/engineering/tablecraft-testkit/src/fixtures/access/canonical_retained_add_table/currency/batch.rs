//! Independent scaled-i64 Currency batches, complete images and protected companions.
use crate::fixtures::access::{
    canonical_retained_add_table::{
        fields, joined_update, populated, typed_fields::Kind, unadorned_fields,
    },
    raw_forwarding_chain,
};
use tablecraft_core::values::{Currency, Value};

#[derive(Clone, Copy)]
pub struct Cell {
    pub row: usize,
    pub expected: Option<i64>,
    pub value: Option<i64>,
}

pub struct Source {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub table: &'static str,
    pub column: &'static str,
    pub cells: Vec<Cell>,
}

pub fn values(original: &[Option<i64>], replacements: &[Option<i64>]) -> Source {
    assert_eq!(original.len(), replacements.len());
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let raw = populated::numeric::source(&seed, "CurrencyValues", 5, original);
    let definition = (seed.len() / 4096) as u32;
    let cells = original
        .iter()
        .zip(replacements)
        .enumerate()
        .map(|(row, (expected, value))| Cell {
            row,
            expected: *expected,
            value: *value,
        })
        .collect::<Vec<_>>();
    finish(raw.bytes, definition, &raw.references, cells)
}

fn finish(before: Vec<u8>, definition: u32, references: &[u32], cells: Vec<Cell>) -> Source {
    // NO_CLONE: BEFORE stays immutable for independent native comparison; the
    // separately mutable raw AFTER changes only literal Currency payload/mask bytes.
    let mut expected = before.to_vec();
    for cell in &cells {
        let reference = references[cell.row];
        let page = reference >> 8;
        let root = (reference & 255) as usize;
        let range = raw_forwarding_chain::terminal_range(&before, page, root);
        let record = &before[range.start..range.end];
        assert_eq!(&record[..2], &[4, 0]);
        let mask = record.len() - 1;
        assert_eq!(record[mask] & 1 != 0, cell.expected.is_some());
        if let Some(value) = cell.expected {
            assert_eq!(&record[2..10], &value.to_le_bytes());
        }
        let changed = &mut expected[range];
        if let Some(value) = cell.value {
            changed[2..10].copy_from_slice(&value.to_le_bytes());
            changed[mask] |= 1;
        } else {
            changed[mask] &= !1;
        }
    }
    assert_eq!(
        &before[definition as usize * 4096..definition as usize * 4096 + 2],
        &[2, 1]
    );
    Source {
        before,
        expected,
        table: "CurrencyValues",
        column: "KeyValue",
        cells,
    }
}

pub fn empty() -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let raw = populated::numeric::source(&seed, "CurrencyValues", 5, &[Some(12345), None]);
    finish(
        raw.bytes,
        (seed.len() / 4096) as u32,
        &raw.references,
        Vec::new(),
    )
}

pub fn forwarded() -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let raw = populated::numeric::source(&seed, "CurrencyValues", 5, &[Some(12345), None]);
    let definition = (seed.len() / 4096) as u32;
    let before = raw_forwarding_chain::extend(&raw.bytes, definition + 1, 0, 20);
    finish(
        before,
        definition,
        &raw.references,
        vec![
            Cell {
                row: 0,
                expected: Some(12345),
                value: None,
            },
            Cell {
                row: 1,
                expected: None,
                value: Some(-1),
            },
        ],
    )
}

pub fn materializing() -> Source {
    let source = joined_update::source();
    let definition = source.target_definition;
    let before = unadorned_fields::after(
        &source.bytes,
        definition,
        "ExtraCurrency",
        Kind::Currency,
        [0; 4],
    );
    let cells = vec![
        Cell {
            row: 0,
            expected: None,
            value: Some(22000),
        },
        Cell {
            row: 1,
            expected: None,
            value: Some(5000),
        },
    ];
    // NO_CLONE: The independently modified materialization oracle retains the
    // original source and its abandoned record history for whole-image comparison.
    let mut expected = before.to_vec();
    for cell in &cells {
        expected = unadorned_fields::rows::update(
            &expected,
            definition,
            cell.row,
            Kind::Currency,
            &Value::Currency(Currency::from_scaled(cell.value.unwrap())),
        );
    }
    Source {
        before,
        expected,
        table: "Table1",
        column: "ExtraCurrency",
        cells,
    }
}

pub fn all_selected() -> Source {
    let original = (0..16_385_i64)
        .map(|row| (row % 17 != 0).then_some(row * 10000 + 1234))
        .collect::<Vec<_>>();
    let replacements = original
        .iter()
        .map(|value| Some(value.unwrap_or(0) + 12500))
        .collect::<Vec<_>>();
    values(&original, &replacements)
}
