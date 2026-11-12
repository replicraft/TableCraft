//! Empty and wholly NULL result sets cannot supply native type or size evidence.
#![cfg(feature = "journal")]

use super::{SCOPE, budget, prepare};
use std::io::Cursor;
use tablecraft_ace::{inspect_table_indexes, inspect_table_schema};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields,
    populated::{
        self,
        computed::{self as raw, Row},
    },
};

#[test]
fn empty_computed_outputs_keep_all_ten_fixed_native_categories_without_observed_values() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(23)]);
    let staged = prepare(
        &before,
        "SELECT CBool(1) AS FlagValue,CByte(1) AS ByteValue,CInt(1) AS ShortValue,CLng(1) AS LongValue,CLngLng(1) AS LargeValue,CSng(1) AS SingleValue,CDbl(1) AS DoubleValue,CCur(1) AS CurrencyValue,CDate(2.5) AS DateValue,{GUID {00112233-4455-6677-8899-aabbccddeeff}} AS Token INTO Archive FROM Items WHERE False",
    )
    .unwrap();
    assert!(staged.rows().is_empty());
    let plan = staged.finish(&mut budget(), &|| false).unwrap();
    let schema = inspect_table_schema(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = [
        (1, 0),
        (2, 1),
        (3, 2),
        (4, 4),
        (19, 8),
        (6, 4),
        (7, 8),
        (5, 8),
        (8, 8),
        (15, 16),
    ];
    assert_eq!(schema.columns().len(), expected.len());
    for (column, (tag, width)) in schema.columns().iter().zip(expected) {
        assert_eq!(column.native_type(), tag, "{}", column.name());
        assert_eq!(column.storage_length(), width, "{}", column.name());
    }
    let indexes = inspect_table_indexes(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        "Archive",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(indexes.indexes().is_empty());
}

#[test]
fn empty_and_all_null_computed_relations_keep_direct_native_text_and_decimal_sizes() {
    let mut seed = fields::source(fields::Case::FirstNumber).bytes;
    for (index, mask) in [1, 27, 152, 253].into_iter().enumerate() {
        seed[110 + index] = [9, 4, 0, 1][index] ^ mask;
    }
    let before = populated::text_decimals(&seed, "Items", &[(None, None), (None, None)]);
    for empty in [false, true] {
        let sql = format!(
            "SELECT i.Label,i.Amount,IIf(i.Label IS NULL,Null,CLng(7)) AS CopiedScore INTO Archive FROM Items AS i{}",
            if empty { " WHERE False" } else { "" }
        );
        let staged = prepare(&before, &sql).unwrap();
        assert_eq!(staged.rows().len(), if empty { 0 } else { 2 });
        assert!(
            staged
                .rows()
                .iter()
                .flatten()
                .all(|value| matches!(value, tablecraft_core::values::Value::Null))
        );
        let rows = [
            Row {
                label: None,
                amount: None,
                score: None,
            },
            Row {
                label: None,
                amount: None,
                score: None,
            },
        ];
        let expected = raw::after(&before, "Archive", if empty { &[] } else { &rows });
        let plan = staged.finish(&mut budget(), &|| false).unwrap();
        assert_eq!(plan.after_bytes().len(), expected.len());
        assert_eq!(
            plan.after_bytes()
                .iter()
                .zip(&expected)
                .position(|(actual, raw)| actual != raw),
            None,
            "first differing complete image byte; empty={empty}"
        );
        let schema = inspect_table_schema(
            &mut Cursor::new(plan.after_bytes()),
            SCOPE,
            "Archive",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns()[0].native_type(), 10);
        assert_eq!(schema.columns()[0].storage_length(), 14);
        assert_eq!(schema.columns()[1].native_type(), 16);
        assert_eq!(schema.columns()[1].decimal_parameters(), Some((28, 4)));
        assert_eq!(schema.columns()[2].native_type(), 4);
        assert_eq!(schema.columns()[2].storage_length(), 4);
    }
}
