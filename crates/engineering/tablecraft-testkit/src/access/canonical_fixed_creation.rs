//! Independent fixed-scalar creation SOURCEs; no production creator or codec.
use super::{canonical_creation, data_page, put16, put32};
use tablecraft_core::values::{Currency, Double, Guid, LegacyDateTime, Single, Value};

const PAGE: usize = 4096;
const USER_DEFINITION_PAGE: usize = 7;
const USER_DATA_PAGE: usize = 8;
const RECORD_BYTES: usize = 63;

/// One independently specified fixed-scalar SOURCE recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// Complete mixed schema without user rows.
    Empty,
    /// Exact numeric extrema, calendar endpoints and asymmetric GUIDs.
    Endpoints,
    /// All-NULL nonBoolean row and alternating shared-mask contributions.
    Nullable,
    /// Distinct positive/negative-zero float and DATE bit representations.
    SignedZero,
    /// Sixty-two complete records; another record cannot fit its remaining 52 bytes.
    Boundary,
}

/// Every SOURCE recipe in its frozen qualification order.
pub const CASES: [Case; 5] = [
    Case::Empty,
    Case::Endpoints,
    Case::Nullable,
    Case::SignedZero,
    Case::Boundary,
];

impl Case {
    /// Stable source identity, without a machine path.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Endpoints => "endpoints",
            Self::Nullable => "nullable",
            Self::SignedZero => "signed-zero",
            Self::Boundary => "boundary",
        }
    }
}

/// One literal native declaration, independent of producer field descriptors.
pub struct Column {
    /// Exact spelling used by the native schema.
    pub name: &'static str,
    /// Native declaration tag, not a shared ScalarType discriminant.
    pub native_type: u8,
    /// Payload offset excluding the two-byte row count prefix.
    pub fixed_offset: u16,
    /// Physical payload width; Boolean has no payload region.
    pub width: u16,
}

/// Ten actual fixed declarations, including deliberately unaligned payloads.
pub const COLUMNS: [Column; 10] = [
    Column {
        name: "Flag",
        native_type: 1,
        fixed_offset: 0,
        width: 0,
    },
    Column {
        name: "ByteValue",
        native_type: 2,
        fixed_offset: 0,
        width: 1,
    },
    Column {
        name: "Int16Value",
        native_type: 3,
        fixed_offset: 1,
        width: 2,
    },
    Column {
        name: "Int32Value",
        native_type: 4,
        fixed_offset: 3,
        width: 4,
    },
    Column {
        name: "Int64Value",
        native_type: 19,
        fixed_offset: 7,
        width: 8,
    },
    Column {
        name: "Money",
        native_type: 5,
        fixed_offset: 15,
        width: 8,
    },
    Column {
        name: "SingleValue",
        native_type: 6,
        fixed_offset: 23,
        width: 4,
    },
    Column {
        name: "DoubleValue",
        native_type: 7,
        fixed_offset: 27,
        width: 8,
    },
    Column {
        name: "DateValue",
        native_type: 8,
        fixed_offset: 35,
        width: 8,
    },
    Column {
        name: "GuidValue",
        native_type: 15,
        fixed_offset: 43,
        width: 16,
    },
];

/// Complete original bytes and independent exact semantic observations.
pub struct Source {
    /// Existing catalog user-table spelling; envelope remains unchanged.
    pub table: &'static str,
    /// Literal native column declarations in declaration order.
    pub columns: &'static [Column],
    /// Exact shared carriers expected from reading the authored records.
    pub rows: Vec<Vec<Value>>,
    /// Complete header-code 5 nine-page source image.
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
enum Cell {
    Null,
    Boolean(bool),
    Byte(u8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Currency(i64),
    Single(u32),
    Double(u64),
    Date(u64),
    Guid([u8; 16]),
}

impl Cell {
    fn value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Boolean(value) => Value::Boolean(value),
            Self::Byte(value) => Value::Byte(value),
            Self::Int16(value) => Value::Int16(value),
            Self::Int32(value) => Value::Int32(value),
            Self::Int64(value) => Value::Int64(value),
            Self::Currency(value) => Value::Currency(Currency::from_scaled(value)),
            Self::Single(bits) => Value::Single(Single::from_bits(bits)),
            Self::Double(bits) => Value::Double(Double::from_bits(bits)),
            Self::Date(bits) => Value::DateTime(LegacyDateTime::from_bits(bits).unwrap()),
            Self::Guid(bytes) => Value::Guid(Guid::from_bytes(bytes)),
        }
    }
}

/// Author one fixed SOURCE from the unchanged qualified canonical envelope.
/// No producer output, SDK template, Decimal/Text/Binary row codec is consumed.
pub fn source(case: Case) -> Source {
    let cells = cells(case);
    let mut bytes = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    let declaration = definition(cells.len());
    bytes[USER_DEFINITION_PAGE * PAGE..(USER_DEFINITION_PAGE + 1) * PAGE]
        .copy_from_slice(&declaration);

    let records = cells.iter().map(|row| (0, record(row))).collect::<Vec<_>>();
    let mut page = data_page(USER_DEFINITION_PAGE as u32, &records);
    let free = PAGE - 14 - records.len() * (2 + RECORD_BYTES);
    put16(&mut page, 2, free as u16);
    bytes[USER_DATA_PAGE * PAGE..(USER_DATA_PAGE + 1) * PAGE].copy_from_slice(&page);

    // The existing envelope represents page 8 as an owned page in every case.
    // Only the already-existing candidate row 7 bit changes at actual page fit.
    let map_page = 3 * PAGE;
    let directory = map_page + 14 + 2 * 7;
    let start = u16::from_le_bytes(bytes[directory..directory + 2].try_into().unwrap()) as usize;
    let candidate = map_page + start + 5 + 1;
    bytes[candidate] = if free >= RECORD_BYTES + 2 { 1 } else { 0 };

    Source {
        table: "Items",
        columns: &COLUMNS,
        rows: cells
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
    put16(&mut bytes, 41, 10);
    put16(&mut bytes, 45, 10);
    put32(&mut bytes, 55, (3 << 8) | 3);
    put32(&mut bytes, 59, (3 << 8) | 7);
    let mut names = 63 + 10 * 25;
    for (number, column) in COLUMNS.iter().enumerate() {
        let descriptor = 63 + 25 * number;
        bytes[descriptor] = column.native_type;
        put32(&mut bytes, descriptor + 1, 1625);
        put16(&mut bytes, descriptor + 5, number as u16);
        put16(&mut bytes, descriptor + 9, number as u16);
        bytes[descriptor + 15] = 3;
        put16(&mut bytes, descriptor + 21, column.fixed_offset);
        put16(&mut bytes, descriptor + 23, column.width);
        let encoded = column
            .name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        put16(&mut bytes, names, encoded.len() as u16);
        names += 2;
        bytes[names..names + encoded.len()].copy_from_slice(&encoded);
        names += encoded.len();
    }
    put16(&mut bytes, names, 0xffff);
    let length = names + 2;
    put32(&mut bytes, 8, length as u32);
    put16(&mut bytes, 2, (PAGE - length - 8) as u16);
    bytes
}

fn record(row: &[Cell; 10]) -> Vec<u8> {
    let mut record = vec![0; RECORD_BYTES];
    put16(&mut record, 0, 10);
    for (number, (cell, column)) in row.iter().zip(&COLUMNS).enumerate() {
        let start = 2 + usize::from(column.fixed_offset);
        let end = start + usize::from(column.width);
        let present = match *cell {
            Cell::Null => {
                assert_ne!(column.native_type, 1, "Boolean has no NULL carrier");
                false
            }
            Cell::Boolean(value) => {
                assert_eq!(column.native_type, 1);
                value
            }
            Cell::Byte(value) => {
                record[start] = value;
                true
            }
            Cell::Int16(value) => {
                record[start..end].copy_from_slice(&value.to_le_bytes());
                true
            }
            Cell::Int32(value) => {
                record[start..end].copy_from_slice(&value.to_le_bytes());
                true
            }
            Cell::Int64(value) | Cell::Currency(value) => {
                record[start..end].copy_from_slice(&value.to_le_bytes());
                true
            }
            Cell::Single(bits) => {
                record[start..end].copy_from_slice(&bits.to_le_bytes());
                true
            }
            Cell::Double(bits) | Cell::Date(bits) => {
                record[start..end].copy_from_slice(&bits.to_le_bytes());
                true
            }
            Cell::Guid(canonical) => {
                // GUID: first 32/16/16-bit words little-endian; last 8 bytes
                // preserve canonical byte order. This is a literal fixture
                // permutation, not uuid or the production fixed codec.
                let native = [
                    canonical[3],
                    canonical[2],
                    canonical[1],
                    canonical[0],
                    canonical[5],
                    canonical[4],
                    canonical[7],
                    canonical[6],
                    canonical[8],
                    canonical[9],
                    canonical[10],
                    canonical[11],
                    canonical[12],
                    canonical[13],
                    canonical[14],
                    canonical[15],
                ];
                record[start..end].copy_from_slice(&native);
                true
            }
        };
        if present {
            record[61 + number / 8] |= 1 << (number % 8);
        }
    }
    record
}

fn cells(case: Case) -> Vec<[Cell; 10]> {
    match case {
        Case::Empty => Vec::new(),
        Case::Endpoints => vec![
            [
                Cell::Boolean(true),
                Cell::Byte(0),
                Cell::Int16(i16::MIN),
                Cell::Int32(i32::MIN),
                Cell::Int64(i64::MIN),
                Cell::Currency(i64::MIN),
                Cell::Single(0xff7f_ffff),
                Cell::Double(0xffef_ffff_ffff_ffff),
                // Literal -657434.0, the supported first whole calendar day.
                Cell::Date(0xc124_1034_0000_0000),
                Cell::Guid([
                    0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc,
                    0xdd, 0xee, 0xff,
                ]),
            ],
            [
                Cell::Boolean(false),
                Cell::Byte(u8::MAX),
                Cell::Int16(i16::MAX),
                Cell::Int32(i32::MAX),
                Cell::Int64(i64::MAX),
                Cell::Currency(i64::MAX),
                Cell::Single(0x7f7f_ffff),
                Cell::Double(0x7fef_ffff_ffff_ffff),
                // Literal 2958465.0, the supported last whole calendar day.
                Cell::Date(0x4146_9240_8000_0000),
                Cell::Guid([0xff; 16]),
            ],
            ordinary(1),
        ],
        Case::Nullable => {
            let mut absent = [Cell::Null; 10];
            absent[0] = Cell::Boolean(false);
            vec![
                absent,
                ordinary(2),
                [
                    Cell::Boolean(false),
                    Cell::Byte(0),
                    Cell::Null,
                    Cell::Int32(0),
                    Cell::Null,
                    Cell::Currency(0),
                    Cell::Null,
                    Cell::Double(0x8000_0000_0000_0000),
                    Cell::Null,
                    Cell::Guid([0; 16]),
                ],
            ]
        }
        Case::SignedZero => {
            let mut positive = ordinary(0);
            positive[6] = Cell::Single(0);
            positive[7] = Cell::Double(0);
            positive[8] = Cell::Date(0);
            let mut negative = positive;
            negative[6] = Cell::Single(0x8000_0000);
            negative[7] = Cell::Double(0x8000_0000_0000_0000);
            negative[8] = Cell::Date(0x8000_0000_0000_0000);
            vec![positive, negative]
        }
        Case::Boundary => (1..=62).map(ordinary).collect(),
    }
}

fn ordinary(row: i32) -> [Cell; 10] {
    [
        Cell::Boolean(row % 2 != 0),
        Cell::Byte(row as u8),
        Cell::Int16(row as i16),
        Cell::Int32(row * 100),
        Cell::Int64(-i64::from(row)),
        Cell::Currency(i64::from(row) * 10_000 + 1),
        Cell::Single(0x3f00_0000),
        Cell::Double(0x3fe0_0000_0000_0000),
        Cell::Date(0xbfd0_0000_0000_0000),
        Cell::Guid([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]),
    ]
}
