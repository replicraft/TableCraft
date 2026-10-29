//! Literal empty source and first-INSERT map initialization gates precede API use.
use super::model as original;
use crate::{
    RowInsert,
    format::{allocation, reader::Reader},
    inspect_table_schema,
};
use std::io::Cursor;
use tablecraft_model::CatalogScope;

mod guards;
mod model;

fn maps(bytes: &[u8], owned: &[u32], hints: &[u32]) {
    let schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut original::budget(),
        &|| false,
    )
    .unwrap();
    let mut source = Cursor::new(bytes);
    let mut budget = original::budget();
    let mut reader = Reader {
        source: &mut source,
        header: schema.header,
        budget: &mut budget,
        cancelled: &|| false,
        reads: 0,
    };
    assert_eq!(schema.layout.usage, 11 << 8);
    assert_eq!(
        allocation::pages(&mut reader, schema.layout.usage).unwrap(),
        owned
    );
    assert_eq!(
        allocation::pages(&mut reader, (11 << 8) | 1).unwrap(),
        hints
    );
}

fn checked(case: &model::Case) {
    let before = original::checked(&case.before, 0, 2);
    assert_eq!(before.rows(), 0);
    assert_eq!(
        &case.before[10 * model::PAGE + 67..10 * model::PAGE + 71],
        &[0; 4]
    );
    original::checked(&case.expected, 1, 2);
    maps(&case.before, &[], &[]);
    maps(&case.expected, &[16], &[16]);
    let owner_start = u16::from_le_bytes(
        case.before[11 * model::PAGE + 14..11 * model::PAGE + 16]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(
        &case.before[11 * model::PAGE + owner_start..11 * model::PAGE + owner_start + 69],
        &[vec![1], vec![0; 68]].concat()
    );
    assert_eq!(
        &case.expected[17 * model::PAGE..17 * model::PAGE + 4],
        &[5, 1, 0, 0]
    );
    assert_eq!(
        &case.before[12 * model::PAGE..13 * model::PAGE],
        &case.expected[12 * model::PAGE..13 * model::PAGE]
    );
    assert_eq!(case.expected[17 * model::PAGE + 6], 1);
    assert_eq!(case.expected[17 * model::PAGE + 6] & 2, 0);
}

#[test]
fn empty_owned_pointer_source_and_first_insert_image_qualify_before_preview() {
    for code in [5, 6] {
        checked(&model::case(code));
    }
}

#[test]
fn first_insert_initializes_missing_ordinary_owned_block_zero() {
    for code in [5, 6] {
        let case = model::case(code);
        checked(&case);
        let candidate = original::capture(&case.before)
            .preview_ace16_insert_rows(
                "Items",
                &[RowInsert::new(case.values)],
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
        let actual = candidate.candidate_bytes();
        let difference = actual
            .iter()
            .zip(&case.expected)
            .position(|(actual, expected)| actual != expected);
        assert_eq!(actual.len(), case.expected.len());
        assert_eq!(
            difference, None,
            "first empty-table INSERT image difference"
        );
    }
}
