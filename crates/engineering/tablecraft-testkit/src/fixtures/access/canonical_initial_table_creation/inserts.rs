//! Literal first-row outcomes for independently authored initial declarations.
use super::{Case, source};
use crate::fixtures::access::{data_page, put16, put32};
use tablecraft_core::values::{Decimal, Double, Guid, Single, Value};

#[derive(Clone, Copy, Debug)]
pub enum InsertCase {
    IdOnly,
    LongInteger,
    LongIntegerNull,
    ShortText,
    ShortTextNull,
    /// Empty supplied Text requires a separately attached AllowZeroLength policy.
    ShortTextEmpty,
}

pub const CASES: [InsertCase; 4] = [
    InsertCase::IdOnly,
    InsertCase::LongInteger,
    InsertCase::ShortText,
    InsertCase::ShortTextNull,
];

pub struct FirstInsert {
    pub declaration: Case,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub companion: Option<Value>,
    pub row: Vec<Value>,
}

/// No production creator, record serializer, key encoder or plan supplies bytes.
pub fn first_insert(case: InsertCase) -> FirstInsert {
    let (declaration, companion, record) = match case {
        InsertCase::IdOnly => (Case::IdOnly, None, vec![1, 0, 1, 0, 0, 0, 1]),
        InsertCase::LongInteger => (
            Case::LongInteger,
            Some(Value::Int32(17)),
            vec![2, 0, 1, 0, 0, 0, 17, 0, 0, 0, 3],
        ),
        InsertCase::LongIntegerNull => (
            Case::LongInteger,
            Some(Value::Null),
            vec![2, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
        ),
        InsertCase::ShortText => (
            Case::ShortText,
            Some(Value::Text("hello".into())),
            vec![
                2, 0, 1, 0, 0, 0, 104, 0, 101, 0, 108, 0, 108, 0, 111, 0, 16, 0, 6, 0, 1, 0, 3,
            ],
        ),
        InsertCase::ShortTextNull => (
            Case::ShortText,
            Some(Value::Null),
            vec![2, 0, 1, 0, 0, 0, 6, 0, 6, 0, 1, 0, 1],
        ),
        InsertCase::ShortTextEmpty => (
            Case::ShortText,
            Some(Value::Text(String::new())),
            vec![2, 0, 1, 0, 0, 0, 6, 0, 6, 0, 1, 0, 3],
        ),
    };
    let before = source(declaration).bytes;
    let after = first_row_image(&before, record);

    let mut row = vec![Value::Int32(1)];
    row.extend(companion.clone());
    FirstInsert {
        declaration,
        before,
        after,
        companion,
        row,
    }
}

/// All six newly admitted ordinary carriers in one literal native row.
/// False selects non-NULL extremes, true the independent nullable outcome.
pub fn number_companions(nulls: bool) -> (Vec<u8>, Vec<u8>, Vec<Value>) {
    let before = super::number_companions().bytes;
    let mut record = vec![7, 0, 1, 0, 0, 0];
    let mut row = vec![Value::Int32(1)];
    if nulls {
        record.extend([0; 31]);
        record.extend([37, 0, 37, 0, 1, 0, 1]);
        row.extend([
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
        ]);
    } else {
        // Byte MAX, Short MIN, Single minimum subnormal, Double negative zero,
        // canonical GUID reordered into the native 4/2/2 little-endian fields.
        record.extend([255, 0, 128, 1, 0, 0, 0]);
        record.extend([0, 0, 0, 0, 0, 0, 0, 128]);
        record.extend([
            51, 34, 17, 0, 85, 68, 119, 102, 136, 153, 170, 187, 204, 221, 238, 255,
        ]);
        // Negative coefficient 123450, scale belongs to the declaration.
        record.extend([128, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 58, 226, 1, 0]);
        record.extend([54, 0, 37, 0, 1, 0, 127]);
        row.extend([
            Value::Byte(255),
            Value::Int16(i16::MIN),
            Value::Single(Single::from_bits(1)),
            Value::Double(Double::from_bits(0x8000_0000_0000_0000)),
            Value::Guid(Guid::from_bytes([
                0, 17, 34, 51, 68, 85, 102, 119, 136, 153, 170, 187, 204, 221, 238, 255,
            ])),
            Value::Decimal(Decimal::new(-123450, 4).unwrap()),
        ]);
    }
    let after = first_row_image(&before, record);
    (before, after, row)
}

/// Independently author a dense generated ID1..N / nullable LONG companion batch.
/// The supplied original is empty; names/properties and free TDEF tails stay opaque.
pub fn long_rows(before: &[u8], values: &[Option<i32>]) -> Vec<u8> {
    assert!((1..=255).contains(&values.len()));
    let definition = &before[7 * 4096..8 * 4096];
    assert_eq!(&definition[41..47], &[2, 0, 0, 0, 2, 0]);
    for (number, flags) in [(0, 7), (1, 3)] {
        let column = 75 + number * 25;
        assert_eq!(definition[column], 4);
        assert_eq!(definition[column + 15], flags);
        assert_eq!(
            &definition[column + 21..column + 25],
            &[(number * 4) as u8, 0, 4, 0]
        );
    }
    let mut records = Vec::new();
    for (position, value) in values.iter().enumerate() {
        let mut record = vec![2, 0];
        record.extend(((position + 1) as i32).to_le_bytes());
        record.extend(value.unwrap_or(0).to_le_bytes());
        record.push(if value.is_some() { 3 } else { 1 });
        records.push((0, record));
    }
    initial_rows_image(before, records)
}

pub(in crate::fixtures::access) fn first_row_image(before: &[u8], record: Vec<u8>) -> Vec<u8> {
    initial_rows_image(before, vec![(0, record)])
}

pub(in crate::fixtures::access) fn initial_rows_image(
    before: &[u8],
    records: Vec<(u16, Vec<u8>)>,
) -> Vec<u8> {
    assert!((1..=255).contains(&records.len()));
    for at in [16, 20, 67] {
        assert_eq!(&before[7 * 4096 + at..7 * 4096 + at + 4], &[0; 4]);
    }
    assert_eq!(&before[8 * 4096 + 12..8 * 4096 + 14], &[0, 0]);
    let mut after = before.to_vec();
    let count = records.len();

    // Declared membership, LAST and physical unique-key history advance together.
    put32(&mut after, 7 * 4096 + 16, count as u32);
    put32(&mut after, 7 * 4096 + 20, count as u32);
    put32(&mut after, 7 * 4096 + 67, count as u32);

    let length: usize = records.iter().map(|(_, record)| record.len()).sum();
    let mut data = data_page(7, &records);
    put16(&mut data, 2, (4096 - 14 - 2 * count - length) as u16);
    after[8 * 4096..9 * 4096].copy_from_slice(&data);

    // Prefix-free ascending signed LONG IDs and their original physical slots.
    let leaf = &mut after[9 * 4096..10 * 4096];
    assert_eq!(&leaf[2..4], &3616_u16.to_le_bytes());
    assert!(leaf[27..480].iter().all(|byte| *byte == 0));
    for position in 0..count {
        let start = 480 + position * 9;
        leaf[start] = 127;
        leaf[start + 1..start + 5]
            .copy_from_slice(&((position as u32 + 1) ^ 0x8000_0000).to_be_bytes());
        leaf[start + 5..start + 9].copy_from_slice(&((8 << 8) | position as u32).to_be_bytes());
        let end = (position + 1) * 9;
        leaf[27 + end / 8] |= 1 << (end % 8);
    }
    put16(leaf, 2, (3616 - count * 9) as u16);
    after
}
