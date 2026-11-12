//! Independent exact-bit fixed-scalar batches beside protected native fields.
use super::{Column, Scalar, append_table, append_values, historical, payload};
use crate::fixtures::access::{canonical_retained_add_table::fields, raw_forwarding_chain};

const PAGE: usize = 4096;
const TABLE: &str = "Measurements";

#[derive(Clone, Copy)]
pub struct Cell {
    pub row: usize,
    pub expected: Option<Scalar>,
    pub value: Option<Scalar>,
}

pub struct Source {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub table: &'static str,
    pub column: Column,
    pub definition: u32,
    pub cells: Vec<Cell>,
}

fn inputs() -> [Scalar; 6] {
    [
        Scalar::Byte(17),
        Scalar::Int16(-1234),
        Scalar::Int64(9_007_199_254_740_993),
        Scalar::SingleBits(0x3fa0_0000),
        Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd),
        Scalar::DateTimeBits(0x4004_0000_0000_0000),
    ]
}

pub fn initial(column: Column) -> Scalar {
    inputs()[column.number() - 2]
}

pub fn replacement(column: Column) -> Scalar {
    match column {
        Column::ByteValue => Scalar::Byte(255),
        Column::ShortValue => Scalar::Int16(i16::MIN),
        Column::LargeValue => Scalar::Int64(9_007_199_254_740_994),
        Column::SingleValue => Scalar::SingleBits(0x8000_0000),
        Column::DoubleValue => Scalar::DoubleBits(1),
        Column::DateValue => Scalar::DateTimeBits((-1.25_f64).to_bits()),
    }
}

pub fn values(
    column: Column,
    original: &[Option<Scalar>],
    replacements: &[Option<Scalar>],
) -> Source {
    assert_eq!(original.len(), replacements.len());
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / PAGE).unwrap();
    let mut before = append_table(&seed, TABLE);
    for original in original {
        let mut values = inputs();
        if let Some(value) = original {
            values[column.number() - 2] = *value;
        }
        before = append_values(&before, definition, &values);
    }
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
    for cell in &cells {
        if cell.expected.is_none() {
            let range = raw_forwarding_chain::terminal_range(&before, definition + 1, cell.row);
            before[range.end - 1] &= !(1 << column.number());
        }
    }
    finish(before, definition, column, cells)
}

fn finish(before: Vec<u8>, definition: u32, column: Column, cells: Vec<Cell>) -> Source {
    let selected = &super::COLUMNS[column.number()];
    let mut expected = before.to_vec();
    for cell in &cells {
        let range = raw_forwarding_chain::terminal_range(&before, definition + 1, cell.row);
        let original = &before[range.start..range.end];
        assert_eq!(&original[..2], &[8, 0]);
        assert_eq!(original.len(), 42);
        let bit = 1 << column.number();
        assert_eq!(original[41] & bit != 0, cell.expected.is_some());
        let offset = 2 + usize::from(selected.fixed_offset.unwrap());
        if let Some(value) = cell.expected {
            let bytes = payload(column, Some(value));
            assert_eq!(&original[offset..offset + bytes.len()], bytes);
        }
        let changed = &mut expected[range];
        if let Some(value) = cell.value {
            let bytes = payload(column, Some(value));
            changed[offset..offset + bytes.len()].copy_from_slice(&bytes);
            changed[41] |= bit;
        } else {
            changed[41] &= !bit;
        }
    }
    Source {
        before,
        expected,
        table: TABLE,
        column,
        definition,
        cells,
    }
}

pub fn empty(column: Column) -> Source {
    let case = values(
        column,
        &[Some(initial(column)), None],
        &[Some(initial(column)), None],
    );
    finish(case.before, case.definition, column, Vec::new())
}

pub fn forwarded(column: Column) -> Source {
    let case = values(
        column,
        &[Some(initial(column)), None],
        &[None, Some(replacement(column))],
    );
    let before = raw_forwarding_chain::extend(&case.before, case.definition + 1, 0, 20);
    finish(before, case.definition, column, case.cells)
}

pub fn materializing(column: Column) -> Source {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(seed.len() / PAGE).unwrap();
    let empty = append_table(&seed, TABLE);
    let first = historical::append(&empty, definition, 17);
    let before = historical::append(&first, definition, 23);
    let cells = vec![
        Cell {
            row: 0,
            expected: None,
            value: Some(initial(column)),
        },
        Cell {
            row: 1,
            expected: None,
            value: Some(replacement(column)),
        },
    ];
    let mut expected = before.to_vec();
    for cell in &cells {
        expected = historical::after(&expected, definition, cell.row, column, cell.value);
    }
    Source {
        before,
        expected,
        table: TABLE,
        column,
        definition,
        cells,
    }
}
