//! Literal first-row outcomes for independently authored initial declarations.
use super::{Case, source};
use crate::fixtures::access::{data_page, put16, put32};
use tablecraft_core::values::Value;

#[derive(Clone, Copy, Debug)]
pub enum InsertCase {
    IdOnly,
    LongInteger,
    ShortText,
    ShortTextNull,
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
    };
    let before = source(declaration).bytes;
    let mut after = before.clone();

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
