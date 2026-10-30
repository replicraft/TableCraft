//! Source/request whole images owned by the existing LVAL literal fixtures.
use super::original;
use std::io::Cursor;
use tablecraft_ace::{LongValueCheckStatus, inspect_table_schema};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

pub(super) struct Case {
    pub before: Vec<u8>,
    pub expected: Vec<u8>,
    pub column: &'static str,
    pub before_value: Value,
    pub value: Value,
}

pub(super) fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    let before = original::null_memo_pressure_source();
    let expected = original::combined_expected(&before);
    cases.push(Case {
        before,
        expected,
        column: "Memo",
        before_value: Value::Null,
        value: Value::Text("Ā".repeat(4073)),
    });

    let before = original::source::source(6, 1);
    let mut expected = before.clone();
    let memo = original::source::single(2, original::source::MEMO, 0);
    let ole = original::inline(&[8]);
    original::source::install(
        &mut expected,
        original::source::ROOT,
        &original::source::page(
            10,
            &[(0, original::source::record(1, Some(&memo), Some(&ole)))],
        ),
    );
    cases.push(Case {
        before,
        expected,
        column: "Ole",
        before_value: Value::Binary(vec![7]),
        value: Value::Binary(vec![8]),
    });

    for (column, empty) in [
        ("Memo", false),
        ("Memo", true),
        ("Ole", false),
        ("Ole", true),
    ] {
        let mut before = original::source::source(6, 1);
        let memo = original::source::single(2, original::source::MEMO, 0);
        let ole = original::source::single(1, original::source::OLE, 0);
        let inline_empty = original::inline(&[]);
        let (memo_value, ole_value) = if column == "Memo" {
            (
                empty.then_some(inline_empty.as_slice()),
                Some(ole.as_slice()),
            )
        } else {
            (
                Some(memo.as_slice()),
                empty.then_some(inline_empty.as_slice()),
            )
        };
        original::source::install(
            &mut before,
            original::source::ROOT,
            &original::source::page(
                10,
                &[(0, original::source::record(1, memo_value, ole_value))],
            ),
        );
        let value = if !empty {
            Value::Null
        } else if column == "Memo" {
            Value::Text(String::new())
        } else {
            Value::Binary(Vec::new())
        };
        cases.push(Case {
            expected: before.clone(),
            before,
            column,
            before_value: value.clone(),
            value,
        });
    }
    cases
}

#[test]
fn qualify_default_long_value_sources_and_expected_before_new_routing() {
    for case in cases() {
        for (bytes, expected_value) in [
            (&case.before, &case.before_value),
            (&case.expected, &case.value),
        ] {
            let schema = inspect_table_schema(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                &mut original::budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                (
                    schema.columns()[1].native_type(),
                    schema.columns()[1].flags(),
                    schema.columns()[1].extended_flags(),
                    schema.columns()[1].storage_length()
                ),
                (12, 2, 0, 0)
            );
            assert_eq!(
                (
                    schema.columns()[2].native_type(),
                    schema.columns()[2].flags(),
                    schema.columns()[2].extended_flags(),
                    schema.columns()[2].storage_length()
                ),
                (11, 2, 0, 0)
            );
            let rows = original::read(bytes);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0][0], Value::Int32(1));
            assert_eq!(
                &rows[0][if case.column == "Memo" { 1 } else { 2 }],
                expected_value
            );
            assert_eq!(
                rows[0][if case.column == "Memo" { 2 } else { 1 }],
                if case.column == "Memo" {
                    Value::Binary(vec![7])
                } else {
                    Value::Text("A".into())
                }
            );
            let checked = original::check(bytes);
            assert!(
                checked
                    .columns()
                    .iter()
                    .all(|column| column.status() == LongValueCheckStatus::Checked)
            );
        }
    }
}
