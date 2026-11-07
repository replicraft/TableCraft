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

fn first_row_image(before: &[u8], record: Vec<u8>) -> Vec<u8> {
    let mut after = before.to_vec();

    // Declared membership, LAST and physical unique-key history each advance once.
    put32(&mut after, 7 * 4096 + 16, 1);
    put32(&mut after, 7 * 4096 + 20, 1);
    put32(&mut after, 7 * 4096 + 67, 1);

    let length = record.len();
    let mut data = data_page(7, &[(0, record)]);
    put16(&mut data, 2, (4096 - 14 - 2 - length) as u16);
    after[8 * 4096..9 * 4096].copy_from_slice(&data);

    // Ascending signed LONG 1 followed by page 8 / slot 0; one complete key.
    let leaf = &mut after[9 * 4096..10 * 4096];
    put16(leaf, 2, 3607);
    leaf[28] = 2;
    leaf[480..489].copy_from_slice(&[127, 128, 0, 0, 1, 0, 0, 8, 0]);

    after
}
