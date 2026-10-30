//! Literal historical geometry and preferred payload controls, without a writer oracle.
use super::{payload, reconstruct};
use crate::EditError;
use crate::InspectError;
use crate::TableSchema;
use crate::WriteBlocker;
use crate::inspect_table_schema;
use crate::table::record_layout;
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::mutation_fixture as base;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 10_000, 1_000_000)
}

fn schema(text: bool, compressed: bool) -> TableSchema {
    let bytes = base::database();
    let mut schema = inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    schema.layout.max_columns = 9;
    schema.layout.variable_slots = 5;
    schema.layout.fixed_end = 6;
    let field = &mut schema.columns[1].field;
    field.kind = if text { 10 } else { 9 };
    field.fixed = false;
    field.flags = 2;
    field.extended_flags = if compressed { 1 } else { 0 };
    field.length = if text { 510 } else { 255 };
    field.number = 8;
    field.variable = 1;
    schema
}

fn original() -> Vec<u8> {
    vec![
        9, 0, 1, 0, 0, 0, 0xa1, 0xa2, 0xa3, 0xb1, 0xb2, 0xb3, 0xb4, 0xd3, 0xe4, 0x91, 0x92, 0x93,
        0x94, 0x95, 0x96, 0x97, 0x98, 15, 0, 13, 0, 9, 0, 6, 0, 3, 0, 0xa5, 0xff,
    ]
}

#[test]
fn historical_slots_masks_and_stationary_padding_determine_capacity() {
    let schema = schema(false, false);
    let before = original();
    for (size, length, end, selected_end) in [(12, 35, 23, 21), (13, 36, 24, 22)] {
        let after = reconstruct(
            &schema,
            1,
            &before,
            &Value::Binary(vec![0x5a; size]),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut expected = vec![9, 0, 1, 0, 0, 0, 0xa1, 0xa2, 0xa3];
        expected.extend(vec![0x5a; size]);
        expected.extend([
            0xd3,
            0xe4,
            end,
            0,
            selected_end,
            0,
            9,
            0,
            6,
            0,
            3,
            0,
            0xa5,
            0xff,
        ]);
        assert_eq!(after.len(), length);
        assert_eq!(after, expected);
    }

    let after = reconstruct(
        &schema,
        1,
        &before,
        &Value::Binary(vec![0x5a]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        after,
        vec![
            9, 0, 1, 0, 0, 0, 0xa1, 0xa2, 0xa3, 0x5a, 0xd3, 0xe4, 0xb4, 0xd3, 0xe4, 0x91, 0x92,
            0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 12, 0, 10, 0, 9, 0, 6, 0, 3, 0, 0xa5, 0xff,
        ]
    );

    let empty = reconstruct(
        &schema,
        1,
        &before,
        &Value::Binary(vec![]),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let null = reconstruct(&schema, 1, &before, &Value::Null, &mut budget(), &|| false).unwrap();
    let mut expected_null = empty.clone();
    expected_null[34] &= !1;
    assert_eq!(null, expected_null);
    assert_eq!(empty.len(), 35);
    assert_eq!(&empty[9..11], &[0xd3, 0xe4]);
    assert_eq!(&empty[23..33], &[11, 0, 9, 0, 9, 0, 6, 0, 3, 0]);
    assert_eq!(&empty[33..], &[0xa5, 0xff]);
}

#[test]
fn preferred_text_is_exact_not_any_semantically_equivalent_slot_encoding() {
    let compressed = schema(true, true);
    for (value, wire) in [
        ("AB", vec![0x41, 0, 0x42, 0]),
        ("ABC", vec![0xff, 0xfe, 0x41, 0x42, 0x43]),
        ("", vec![]),
        ("A\0B", vec![0x41, 0, 0, 0, 0x42, 0]),
        ("\u{100}BC", vec![0, 1, 0x42, 0, 0x43, 0]),
        ("😀", vec![0x3d, 0xd8, 0, 0xde]),
    ] {
        let actual = payload::encode(
            &compressed.columns[1],
            &Value::Text(value.into()),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(actual, wire, "{value:?}");
    }

    let raw = schema(true, false);
    assert_eq!(
        payload::encode(
            &raw.columns[1],
            &Value::Text("ABC".into()),
            &mut budget(),
            &|| false
        )
        .unwrap(),
        vec![0x41, 0, 0x42, 0, 0x43, 0]
    );
    assert_eq!(
        payload::encode(
            &raw.columns[1],
            &Value::Text("\u{feff}A".into()),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    );
}

#[test]
fn historical_count_refusal_and_resource_failures_publish_no_record() {
    let mut schema = schema(false, false);
    schema.layout.max_columns = 10;
    schema.layout.variable_slots = 4;
    record_layout::parse(&original(), &schema, &mut budget()).unwrap();
    assert_eq!(
        reconstruct(
            &schema,
            1,
            &original(),
            &Value::Binary(vec![0x5a]),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    );

    let schema = self::schema(false, false);
    for (bytes, steps) in [(0, 1_000_000), (16 * 1024 * 1024, 0)] {
        let error = reconstruct(
            &schema,
            1,
            &original(),
            &Value::Binary(vec![0x5a]),
            &mut ResourceBudget::new(bytes, 10_000, steps),
            &|| false,
        )
        .unwrap_err();
        assert!(matches!(error, EditError::Inspect(InspectError::Limit(_))));
    }
    let error = reconstruct(
        &schema,
        1,
        &original(),
        &Value::Binary(vec![0x5a]),
        &mut budget(),
        &|| true,
    )
    .unwrap_err();
    assert_eq!(error, EditError::Inspect(InspectError::Cancelled));
}
