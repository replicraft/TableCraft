use super::super::{Family, proof};
use crate::{
    ConstraintViolation, EditError, FittedColumnUpdates, InspectError, NativeImage,
    SlotScalarUpdate, WriteBlocker, inspect_table_properties,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::FieldConstraintViolation;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    declared_fields, fields, typed_fields,
    typed_fields::Kind,
    unadorned_fields::{self, rows},
};

const SCOPE: CatalogScope = CatalogScope::new(3718);
const TEXT: Kind = Kind::ShortText {
    units: 80,
    compression: false,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}
fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None
    );
}

#[test]
fn short_text_empty_without_azl_refuses_producer_and_fresh_proof_even_for_equal_input() {
    let source = declared_fields::source();
    let before = unadorned_fields::after(
        &source.bytes,
        source.definition,
        "AddedValue",
        TEXT,
        [9, 4, 0, 1],
    );
    let empty = Value::Text(String::new());
    let raw_after = rows::update(&before, source.definition, 0, TEXT, &empty);
    for (before, expected) in [
        (&before, Value::Null),
        (&raw_after, Value::Text(String::new())),
    ] {
        let updates = [SlotScalarUpdate::new(
            0,
            expected,
            Value::Text(String::new()),
        )];
        let groups = [FittedColumnUpdates::Variable {
            column: "AddedValue",
            updates: &updates,
        }];
        assert!(matches!(
            image(before).plan_retained_row_updates("Items", &groups, &mut budget(), &|| false),
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
        assert!(matches!(
            proof::prove(
                &image(before),
                "Items",
                &groups,
                &raw_after,
                Family::Retained,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
        ));
    }
    for value in [Value::Text("present 🦀".into()), Value::Null] {
        let expected = rows::update(&before, source.definition, 0, TEXT, &value);
        let updates = [SlotScalarUpdate::new(0, Value::Null, value)];
        let groups = [FittedColumnUpdates::Variable {
            column: "AddedValue",
            updates: &updates,
        }];
        let plan = image(&before)
            .plan_retained_row_updates("Items", &groups, &mut budget(), &|| false)
            .unwrap();
        same(plan.after_bytes(), &expected);
    }
}

#[test]
fn explicit_azl_true_keeps_empty_text_while_false_uses_normal_constraint_validation() {
    let source = fields::source(fields::Case::FirstNumber);
    let before = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "AddedValue",
        TEXT,
    );
    let expected = rows::update(
        &before,
        source.definition,
        source.roots[0],
        TEXT,
        &Value::Text(String::new()),
    );
    let updates = [SlotScalarUpdate::new(
        0,
        Value::Null,
        Value::Text(String::new()),
    )];
    let groups = [FittedColumnUpdates::Variable {
        column: "AddedValue",
        updates: &updates,
    }];
    let plan = image(&before)
        .plan_retained_row_updates("Table1", &groups, &mut budget(), &|| false)
        .unwrap();
    same(plan.after_bytes(), &expected);

    let properties = inspect_table_properties(
        &mut Cursor::new(&before),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == "AddedValue")
        .unwrap();
    assert!(group.get("AllowZeroLength").unwrap().boolean().unwrap());
    let body = properties.raw_bytes();
    assert_eq!(body.last(), Some(&255));
    // The independent gallery fixture appends Required then AZL in the final
    // group. Change only its literal one-byte Boolean, preserving every frame.
    let locations: Vec<_> = before
        .windows(body.len())
        .enumerate()
        .filter_map(|(at, part)| (part == body).then_some(at))
        .collect();
    assert_eq!(locations.len(), 1);
    let mut disallow = before.to_vec();
    disallow[locations[0] + body.len() - 1] = 0;
    let properties = inspect_table_properties(
        &mut Cursor::new(&disallow),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == "AddedValue")
        .unwrap();
    assert!(!group.get("AllowZeroLength").unwrap().boolean().unwrap());
    assert!(matches!(
        image(&disallow).plan_retained_row_updates("Table1", &groups, &mut budget(), &|| false),
        Err(EditError::Inspect(InspectError::Constraint(
            ConstraintViolation::Field(FieldConstraintViolation::EmptyText)
        )))
    ));
}
