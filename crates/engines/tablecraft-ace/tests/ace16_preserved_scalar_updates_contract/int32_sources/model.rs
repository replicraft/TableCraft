//! Additive scalar-declaration derivative of the actual rich Boolean source owner.
use super::rich;

pub(crate) const CASES: [(u8, i32); 8] = [
    (5, 101),
    (5, -101),
    (5, 2_147_483_647),
    (5, -2_147_483_648),
    (6, 101),
    (6, -101),
    (6, 2_147_483_647),
    (6, -2_147_483_648),
];

const PAGE_BYTES: usize = 4096;
const TABLE_DEFINITION: usize = 10;
const FIELD_DESCRIPTORS: usize = 99;
const FIELD_BYTES: usize = 25;
const TARGET_NUMBER: usize = 4;
const FIXED_WIDTH_OFFSET: usize = 23;
const DATA_PAGE: usize = 12;
const FIRST_ROW_DIRECTORY: usize = 14;
const RECORD_COLUMN_COUNT_BYTES: usize = 2;
const TARGET_FIXED_OFFSET: usize = 8;

pub(crate) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub patch: usize,
    pub value: i32,
}

pub(crate) fn source(code: u8, value: i32) -> Case {
    assert!(matches!(code, 5 | 6));
    let mut before = rich::source().before;
    assert_eq!(before.len(), 32 * PAGE_BYTES);
    assert_eq!(before[20], 6);
    before[20] = code;

    // The original rich source has only indexed Int32 fields (ID and Number).
    // Its unindexed Large_Number field is an ordinary fixed Int64. This explicit
    // independent source derivative declares that same position as Int32/width4;
    // its old upper four bytes become untouched fixed-area padding. No indexes,
    // calculations, properties, relationships, maps or record layout are changed.
    let field = TABLE_DEFINITION * PAGE_BYTES + FIELD_DESCRIPTORS + TARGET_NUMBER * FIELD_BYTES;
    assert_eq!(before[field], 19);
    assert_eq!(&before[field + 5..field + 7], &4_u16.to_le_bytes());
    assert_eq!(&before[field + 15..field + 17], &[3, 0]);
    assert_eq!(&before[field + 21..field + 23], &8_u16.to_le_bytes());
    assert_eq!(
        &before[field + FIXED_WIDTH_OFFSET..field + FIXED_WIDTH_OFFSET + 2],
        &8_u16.to_le_bytes()
    );
    before[field] = 4;
    before[field + FIXED_WIDTH_OFFSET..field + FIXED_WIDTH_OFFSET + 2]
        .copy_from_slice(&4_u16.to_le_bytes());

    let page = DATA_PAGE * PAGE_BYTES;
    let start = usize::from(u16::from_le_bytes(
        before[page + FIRST_ROW_DIRECTORY..page + FIRST_ROW_DIRECTORY + 2]
            .try_into()
            .unwrap(),
    ));
    assert_eq!(start & 0xe000, 0);
    let patch = page + start + RECORD_COLUMN_COUNT_BYTES + TARGET_FIXED_OFFSET;
    assert_eq!(&before[patch..patch + 8], &1000_i64.to_le_bytes());
    let mut expected = before.clone();
    expected[patch..patch + 4].copy_from_slice(&value.to_le_bytes());
    assert_eq!(&before[..patch], &expected[..patch]);
    assert_eq!(&before[patch + 4..], &expected[patch + 4..]);

    Case {
        before,
        expected,
        patch,
        value,
    }
}
