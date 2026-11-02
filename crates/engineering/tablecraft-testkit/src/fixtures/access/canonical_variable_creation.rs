//! Independent variable scalar SOURCEs; no production serializer or codec import.
use super::{canonical_creation, data_page, put16, put32};
use tablecraft_core::values::{Decimal, Value};

const PAGE: usize = 4096;
const DECIMAL_MAX: i128 = 9_999_999_999_999_999_999_999_999_999;
const ZERO: [u8; 17] = [0; 17];
const POSITIVE_MAX: [u8; 17] = [
    0, 0, 0, 0, 0, 0x5e, 0xce, 0x4f, 0x20, 0x61, 0x02, 0x25, 0x3e, 0xff, 0xff, 0xff, 0x0f,
];
const NEGATIVE_MAX: [u8; 17] = [
    0x80, 0, 0, 0, 0, 0x5e, 0xce, 0x4f, 0x20, 0x61, 0x02, 0x25, 0x3e, 0xff, 0xff, 0xff, 0x0f,
];
const NEGATIVE_ONE_POINT_0001: [u8; 17] =
    [0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x11, 0x27, 0, 0];
const WORD_ORDER: [u8; 17] = [0, 0, 0, 0, 0, 4, 3, 2, 1, 8, 7, 6, 5, 12, 11, 10, 9];

/// Closed positive SOURCE recipes; raw BOM aliasing has a separate reader control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Declared Text/Binary/Decimal schema with no rows.
    Empty,
    /// NULL and present-empty carriers with distinct mask contributions.
    NullAndEmpty,
    /// UTF-16/NUL/non-BMP text, arbitrary binary and Decimal word-order witnesses.
    Unicode,
    /// Exact positive/negative precision endpoints and retained-scale zero.
    DecimalSigns,
    /// Leading U+FEFF with explicit mixed-width text representation.
    EscapedBom,
}

/// All positive SOURCEs in their frozen qualification order.
pub const CASES: [Case; 5] = [
    Case::Empty,
    Case::NullAndEmpty,
    Case::Unicode,
    Case::DecimalSigns,
    Case::EscapedBom,
];

impl Case {
    /// Stable source identity, not a path or payload-derived name.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::NullAndEmpty => "null-empty",
            Self::Unicode => "unicode",
            Self::DecimalSigns => "decimal-signs",
            Self::EscapedBom => "escaped-bom",
        }
    }
}

/// Explicit native declaration facts; Text width is twice its UTF-16 unit limit.
pub struct Column {
    /// Literal field name.
    pub name: &'static str,
    /// Native type tag, independent of producer enums.
    pub native_type: u8,
    /// Fixed payload offset, or None for a variable field.
    pub fixed_offset: Option<u16>,
    /// Declared native byte length, not the encoded variable payload length.
    pub width: u16,
    /// SDK variable-table index, including the unused current index for fixed fields.
    pub variable_index: u16,
    /// Native Decimal precision; zero for other families.
    pub precision: u8,
    /// Native Decimal retained scale; zero for other families.
    pub scale: u8,
}

/// One closed five-field schema with three ordinary variable slots and no LVALs.
pub const COLUMNS: [Column; 5] = [
    Column {
        name: "Id",
        native_type: 4,
        fixed_offset: Some(0),
        width: 4,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "TextValue",
        native_type: 10,
        fixed_offset: None,
        width: 16,
        variable_index: 0,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "BinaryValue",
        native_type: 9,
        fixed_offset: None,
        width: 16,
        variable_index: 1,
        precision: 0,
        scale: 0,
    },
    Column {
        name: "DecimalValue",
        native_type: 16,
        fixed_offset: None,
        width: 17,
        variable_index: 2,
        precision: 28,
        scale: 4,
    },
    Column {
        name: "Flag",
        native_type: 1,
        fixed_offset: Some(0),
        width: 0,
        variable_index: 3,
        precision: 0,
        scale: 0,
    },
];

/// Complete independent bytes and exact semantic observations.
pub struct Source {
    /// Unchanged user table in the qualified canonical envelope.
    pub table: &'static str,
    /// Literal declaration metadata in field order.
    pub columns: &'static [Column],
    /// Expected exact stored carriers, with NULL distinct from empty values.
    pub rows: Vec<Vec<Value>>,
    /// Complete original file, not a production-generated candidate.
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Cell {
    Null,
    Int32(i32),
    Text(&'static str, &'static [u8]),
    Binary(&'static [u8]),
    Decimal(i128, &'static [u8; 17]),
    Boolean(bool),
}

impl Cell {
    fn value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Int32(value) => Value::Int32(value),
            Self::Text(text, _) => Value::Text(text.into()),
            Self::Binary(bytes) => Value::Binary(bytes.to_vec()),
            Self::Decimal(coefficient, _) => Value::Decimal(Decimal::new(coefficient, 4).unwrap()),
            Self::Boolean(value) => Value::Boolean(value),
        }
    }

    fn payload(self) -> &'static [u8] {
        match self {
            Self::Null => &[],
            Self::Text(_, bytes) | Self::Binary(bytes) => bytes,
            Self::Decimal(_, bytes) => bytes,
            _ => panic!("fixed fixture cell has no variable payload"),
        }
    }
}

/// Construct only a closed independent SOURCE; no production codec executes.
pub fn source(case: Case) -> Source {
    build(cells(case))
}

/// A reader-only alias control, NOT a canonical whole-image creation expectation.
/// Raw FF FE is observed as empty Text, not the intended leading U+FEFF character.
pub fn raw_bom_alias_source() -> Source {
    build(vec![[
        Cell::Int32(1),
        Cell::Text("", &[0xff, 0xfe]),
        Cell::Null,
        Cell::Null,
        Cell::Boolean(false),
    ]])
}

fn build(rows: Vec<[Cell; 5]>) -> Source {
    let mut bytes = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    bytes[7 * PAGE..8 * PAGE].copy_from_slice(&definition(rows.len()));
    let records = rows.iter().map(|row| (0, record(row))).collect::<Vec<_>>();
    let used = records.iter().map(|(_, row)| row.len() + 2).sum::<usize>();
    let mut data = data_page(7, &records);
    put16(&mut data, 2, (PAGE - 14 - used) as u16);
    bytes[8 * PAGE..9 * PAGE].copy_from_slice(&data);
    // All selected sources leave ample room for the largest declared/escaped
    // complete row. Existing candidate/owned page 8 membership stays unchanged.
    assert!(PAGE - 14 - used >= 71);
    Source {
        table: "Items",
        columns: &COLUMNS,
        rows: rows
            .into_iter()
            .map(|row| row.into_iter().map(Cell::value).collect())
            .collect(),
        bytes,
    }
}

fn definition(rows: usize) -> Vec<u8> {
    let mut bytes = vec![0; PAGE];
    bytes[..2].copy_from_slice(&[2, 1]);
    put32(&mut bytes, 12, 1625);
    put32(&mut bytes, 16, rows as u32);
    bytes[24] = 1;
    bytes[40] = 0x4e;
    put16(&mut bytes, 41, 5);
    put16(&mut bytes, 43, 3);
    put16(&mut bytes, 45, 5);
    put32(&mut bytes, 55, (3 << 8) | 3);
    put32(&mut bytes, 59, (3 << 8) | 7);
    let mut names = 63 + 5 * 25;
    for (number, column) in COLUMNS.iter().enumerate() {
        let descriptor = 63 + 25 * number;
        bytes[descriptor] = column.native_type;
        put32(&mut bytes, descriptor + 1, 1625);
        put16(&mut bytes, descriptor + 5, number as u16);
        put16(&mut bytes, descriptor + 7, column.variable_index);
        put16(&mut bytes, descriptor + 9, number as u16);
        if column.native_type == 10 {
            bytes[descriptor + 11..descriptor + 15].copy_from_slice(&[9, 4, 0, 1]);
        } else {
            bytes[descriptor + 11] = column.precision;
            bytes[descriptor + 12] = column.scale;
        }
        bytes[descriptor + 15] = if column.fixed_offset.is_some() { 3 } else { 2 };
        put16(
            &mut bytes,
            descriptor + 21,
            column.fixed_offset.unwrap_or(0),
        );
        put16(&mut bytes, descriptor + 23, column.width);
        let name = column
            .name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put16(&mut bytes, names, name.len() as u16);
        names += 2;
        bytes[names..names + name.len()].copy_from_slice(&name);
        names += name.len();
    }
    put16(&mut bytes, names, 0xffff);
    let length = names + 2;
    put32(&mut bytes, 8, length as u32);
    put16(&mut bytes, 2, (PAGE - length - 8) as u16);
    bytes
}

fn record(row: &[Cell; 5]) -> Vec<u8> {
    let mut bytes = vec![0; 6];
    put16(&mut bytes, 0, 5);
    let mut present = 0_u8;
    match row[0] {
        Cell::Int32(id) => {
            bytes[2..6].copy_from_slice(&id.to_le_bytes());
            present |= 1;
        }
        Cell::Null => {}
        _ => panic!("Id fixture requires Int32 or NULL"),
    }
    let mut starts = [0_u16; 3];
    for (slot, start) in starts.iter_mut().enumerate() {
        *start = bytes.len() as u16;
        let cell = row[slot + 1];
        if !matches!(cell, Cell::Null) {
            present |= 1 << (slot + 1);
            bytes.extend_from_slice(cell.payload());
        }
    }
    let Cell::Boolean(flag) = row[4] else {
        panic!("Boolean has no NULL payload");
    };
    if flag {
        present |= 0x10;
    }
    let eod = bytes.len() as u16;
    bytes.extend_from_slice(&eod.to_le_bytes());
    for start in starts.into_iter().rev() {
        bytes.extend_from_slice(&start.to_le_bytes());
    }
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.push(present);
    bytes
}

fn cells(case: Case) -> Vec<[Cell; 5]> {
    match case {
        Case::Empty => Vec::new(),
        Case::NullAndEmpty => vec![
            [
                Cell::Int32(1),
                Cell::Null,
                Cell::Null,
                Cell::Null,
                Cell::Boolean(false),
            ],
            [
                Cell::Int32(2),
                Cell::Text("", &[]),
                Cell::Binary(&[]),
                Cell::Decimal(0, &ZERO),
                Cell::Boolean(true),
            ],
            [
                Cell::Int32(3),
                Cell::Null,
                Cell::Binary(&[]),
                Cell::Null,
                Cell::Boolean(true),
            ],
            [
                Cell::Int32(4),
                Cell::Text("", &[]),
                Cell::Null,
                Cell::Null,
                Cell::Boolean(false),
            ],
        ],
        Case::Unicode => vec![
            [
                Cell::Int32(1),
                Cell::Text(
                    "Ω雪🙂a",
                    &[0xa9, 0x03, 0xea, 0x96, 0x3d, 0xd8, 0x42, 0xde, 0x61, 0],
                ),
                Cell::Binary(&[0, 0xff, 0x41, 0, 0x80]),
                Cell::Decimal(-10_001, &NEGATIVE_ONE_POINT_0001),
                Cell::Boolean(true),
            ],
            [
                Cell::Int32(2),
                Cell::Text("A\0B", &[0x41, 0, 0, 0, 0x42, 0]),
                Cell::Binary(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]),
                Cell::Decimal(311_917_102_708_983_781_990_730_508, &WORD_ORDER),
                Cell::Boolean(false),
            ],
            [
                Cell::Int32(3),
                Cell::Text(
                    "abcdefgh",
                    &[
                        b'a', 0, b'b', 0, b'c', 0, b'd', 0, b'e', 0, b'f', 0, b'g', 0, b'h', 0,
                    ],
                ),
                Cell::Binary(&[0xff; 16]),
                Cell::Decimal(DECIMAL_MAX, &POSITIVE_MAX),
                Cell::Boolean(false),
            ],
        ],
        Case::DecimalSigns => vec![
            [
                Cell::Int32(1),
                Cell::Null,
                Cell::Null,
                Cell::Decimal(DECIMAL_MAX, &POSITIVE_MAX),
                Cell::Boolean(true),
            ],
            [
                Cell::Int32(2),
                Cell::Null,
                Cell::Null,
                Cell::Decimal(-DECIMAL_MAX, &NEGATIVE_MAX),
                Cell::Boolean(false),
            ],
            [
                Cell::Int32(3),
                Cell::Null,
                Cell::Null,
                Cell::Decimal(0, &ZERO),
                Cell::Boolean(true),
            ],
        ],
        Case::EscapedBom => vec![
            [
                Cell::Int32(1),
                Cell::Text("\u{feff}", &[0xff, 0xfe, 0, 0xff, 0xfe]),
                Cell::Null,
                Cell::Null,
                Cell::Boolean(false),
            ],
            [
                Cell::Int32(2),
                Cell::Text(
                    "\u{feff}abcdefg",
                    &[
                        0xff, 0xfe, 0, 0xff, 0xfe, b'a', 0, b'b', 0, b'c', 0, b'd', 0, b'e', 0,
                        b'f', 0, b'g', 0,
                    ],
                ),
                Cell::Binary(&[0xff, 0xfe, 0, 0xff]),
                Cell::Null,
                Cell::Boolean(true),
            ],
        ],
    }
}
