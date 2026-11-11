//! Non-tail removal preserves stored variable slots and Boolean value-mask bits.
use super::{PAGE, SCOPE, budget, image};
use crate::{FieldDropError, inspect_table_schema, read_table};
use std::io::Cursor;
use tablecraft_core::values::{Currency, Decimal, Guid, Value};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    boolean, exact_values, field_changes, fields, typed_fields,
};

fn origin(bytes: &[u8]) -> (u32, usize) {
    (
        (bytes.len() / PAGE) as u32,
        u16::from_le_bytes(bytes[4 * PAGE + 12..4 * PAGE + 14].try_into().unwrap()) as usize,
    )
}

fn assert_drop(
    before: &[u8],
    definition: u32,
    catalog_root: usize,
    column: &str,
    removed_value: Value,
    names: &[(&str, usize)],
    rows: &[Vec<Value>],
) {
    let original_schema = inspect_table_schema(
        &mut Cursor::new(before),
        SCOPE,
        "Measurements",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let selected = original_schema
        .columns()
        .iter()
        .position(|field| field.name() == column)
        .unwrap();
    assert_eq!(
        read_table(
            &mut Cursor::new(before),
            SCOPE,
            "Measurements",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows()[0][selected],
        removed_value
    );
    let expected = field_changes::drop_field(before, definition, catalog_root, column);
    let candidate = image(before)
        .prepare_retained_non_tail_field_drop("Measurements", column, &mut budget(), &|| false)
        .expect("new non-tail language must produce the independently authored AFTER");
    assert_eq!(candidate.after_bytes().len(), expected.len());
    assert!(
        candidate.after_bytes() == expected,
        "first difference {:?}",
        candidate
            .after_bytes()
            .iter()
            .zip(&expected)
            .position(|(actual, expected)| actual != expected)
    );
    let schema = inspect_table_schema(
        &mut Cursor::new(&expected),
        SCOPE,
        "Measurements",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|field| (field.name(), field.number()))
            .collect::<Vec<_>>(),
        names
    );
    assert_eq!(
        read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Measurements",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        rows
    );
    assert_storage(before, &expected, definition as usize);
    assert!(matches!(
        image(before).prepare_retained_tail_field_drop(
            "Measurements",
            column,
            &mut budget(),
            &|| false
        ),
        Err(FieldDropError::Source)
    ));
    assert!(matches!(
        image(before).prepare_retained_typed_tail_field_drop(
            "Measurements",
            column,
            &mut budget(),
            &|| false
        ),
        Err(FieldDropError::Source)
    ));
}

fn assert_storage(before: &[u8], expected: &[u8], definition: usize) {
    assert_eq!(
        &expected[definition * PAGE + 41..definition * PAGE + 45],
        &before[definition * PAGE + 41..definition * PAGE + 45],
        "native high water"
    );
    for page in 0..before.len() / PAGE {
        if ![4, 10, definition].contains(&page) {
            assert!(
                expected[page * PAGE..(page + 1) * PAGE] == before[page * PAGE..(page + 1) * PAGE],
                "protected page {page}"
            );
        }
    }
}

#[test]
fn retained_middle_text_keeps_nonempty_retired_variable_payload_and_boolean_survivor() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let (definition, catalog_root) = origin(&seed);
    let created = boolean::mixed::append_table(&seed, "Measurements");
    let inserted = boolean::mixed::append(&created, definition);
    let text = boolean::mixed::update_text(&inserted, definition, 0, Some("日本語🦀 retained"));
    let before = boolean::mixed::update_bool(&text, definition, 0, true);
    assert_drop(
        &before,
        definition,
        catalog_root,
        "Label",
        Value::Text("日本語🦀 retained".into()),
        &[("ID", 0), ("Score", 1), ("Amount", 2), ("Enabled", 4)],
        &[vec![
            Value::Int32(1),
            Value::Int32(0),
            Value::Currency(Currency::from_scaled(0)),
            Value::Boolean(true),
        ]],
    );
}

#[test]
fn retained_middle_decimal_keeps_exact_payload_and_retained_slot_after_later_fixed_add() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let (definition, catalog_root) = origin(&seed);
    let created = exact_values::append_table(&seed, "Measurements");
    let guid = [0x31; 16];
    let inserted = exact_values::append(
        &created,
        definition,
        Some(guid),
        Some(-90_071_992_547_409_934_321),
    );
    let before = typed_fields::after(
        &inserted,
        definition,
        catalog_root,
        "Marker",
        typed_fields::Kind::Int32,
    );
    assert_drop(
        &before,
        definition,
        catalog_root,
        "Amount",
        Value::Decimal(Decimal::new(-90_071_992_547_409_934_321, 4).unwrap()),
        &[("ID", 0), ("Score", 1), ("Token", 2), ("Marker", 4)],
        &[vec![
            Value::Int32(1),
            Value::Int32(0),
            Value::Guid(Guid::from_bytes(guid)),
            Value::Null,
        ]],
    );
}

#[test]
fn retained_middle_boolean_removes_no_payload_and_keeps_text_and_mask_neighbors() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let (definition, catalog_root) = origin(&seed);
    let created = boolean::mixed::append_table(&seed, "Measurements");
    let inserted = boolean::mixed::append(&created, definition);
    let text = boolean::mixed::update_text(&inserted, definition, 0, Some("not a Boolean bit"));
    let checked = boolean::mixed::update_bool(&text, definition, 0, true);
    let before = typed_fields::after(
        &checked,
        definition,
        catalog_root,
        "Marker",
        typed_fields::Kind::Int32,
    );
    assert_drop(
        &before,
        definition,
        catalog_root,
        "Enabled",
        Value::Boolean(true),
        &[
            ("ID", 0),
            ("Score", 1),
            ("Amount", 2),
            ("Label", 3),
            ("Marker", 5),
        ],
        &[vec![
            Value::Int32(1),
            Value::Int32(0),
            Value::Currency(Currency::from_scaled(0)),
            Value::Text("not a Boolean bit".into()),
            Value::Null,
        ]],
    );
}

#[test]
fn retained_middle_keyless_ordinary_zero_is_not_a_generated_primary_identity() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let (definition, catalog_root) = origin(&seed);
    let before = field_changes::keyless_first_field(&seed, "Keyless");
    let base = definition as usize * PAGE;
    // Literal source facts: no logical/physical index, LONG0, updatable fixed,
    // no generation flags. Its value17 must disappear; Score23 must survive.
    assert_eq!(&before[base + 47..base + 55], &[0; 8]);
    assert_eq!(before[base + 63], 4);
    assert_eq!(&before[base + 68..base + 70], &[0, 0]);
    assert_eq!(&before[base + 78..base + 80], &[3, 0]);
    assert_eq!(
        read_table(
            &mut Cursor::new(&before),
            SCOPE,
            "Keyless",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(17), Value::Int32(23)]]
    );
    let expected = field_changes::drop_field(&before, definition, catalog_root, "Id");
    let candidate = image(&before)
        .prepare_retained_non_tail_field_drop("Keyless", "Id", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.identity().column_number(), 0);
    assert_eq!(candidate.after_bytes().len(), expected.len());
    assert!(
        candidate.after_bytes() == expected,
        "first difference {:?}",
        candidate
            .after_bytes()
            .iter()
            .zip(&expected)
            .position(|(a, b)| a != b)
    );
    assert_eq!(
        read_table(
            &mut Cursor::new(&expected),
            SCOPE,
            "Keyless",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(23)]]
    );
    let schema = inspect_table_schema(
        &mut Cursor::new(&expected),
        SCOPE,
        "Keyless",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        schema
            .columns()
            .iter()
            .map(|field| (field.name(), field.number()))
            .collect::<Vec<_>>(),
        [("Score", 1)]
    );
    assert_storage(&before, &expected, definition as usize);
    assert!(matches!(
        image(&before).prepare_retained_typed_tail_field_drop(
            "Keyless",
            "Id",
            &mut budget(),
            &|| false
        ),
        Err(FieldDropError::Source)
    ));

    let generated = fields::source(fields::Case::FirstNumber);
    assert!(
        std::panic::catch_unwind(|| field_changes::drop_field(
            &generated.bytes,
            generated.definition,
            generated.catalog_root,
            "ID"
        ))
        .is_err()
    );
    assert!(matches!(
        image(&generated.bytes).prepare_retained_non_tail_field_drop(
            "Table1",
            "ID",
            &mut budget(),
            &|| false
        ),
        Err(FieldDropError::Source)
    ));
}
