//! Independent all-kind metadata and fresh-default records, without old-row backfill.
use crate::{
    CreateColumn, CreateDatabaseError, CurrentFieldAdditionError, InsertFieldValue, NativeImage,
    OmittedRowInsert, RetainedFieldAdditionDefinition, RetainedFieldAdditionType, read_table,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, Single, Value},
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    fields, typed_fields as raw,
};

const SCOPE: CatalogScope = CatalogScope::new(4212);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first differing offset {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected)
    );
}

fn scalar(kind: raw::Kind) -> ScalarType {
    match kind {
        raw::Kind::Boolean => ScalarType::Boolean,
        raw::Kind::Byte => ScalarType::Byte,
        raw::Kind::Int16 => ScalarType::Int16,
        raw::Kind::Int32 => ScalarType::Int32,
        raw::Kind::Int64 => ScalarType::Int64,
        raw::Kind::Single => ScalarType::Single,
        raw::Kind::Double => ScalarType::Double,
        raw::Kind::Currency => ScalarType::Currency,
        raw::Kind::DateTime => ScalarType::DateTime,
        raw::Kind::Guid => ScalarType::Guid,
        raw::Kind::Decimal { .. } => ScalarType::Decimal,
        raw::Kind::ShortText { .. } => ScalarType::Text,
    }
}

fn default_value(kind: raw::Kind) -> Option<Value> {
    match kind {
        raw::Kind::Boolean => Some(Value::Boolean(false)),
        raw::Kind::Byte => Some(Value::Byte(0)),
        raw::Kind::Int16 => Some(Value::Int16(0)),
        raw::Kind::Int32 => Some(Value::Int32(0)),
        raw::Kind::Int64 => Some(Value::Int64(0)),
        raw::Kind::Single => Some(Value::Single(Single::from_bits(0))),
        raw::Kind::Double => Some(Value::Double(Double::from_bits(0))),
        raw::Kind::Currency => Some(Value::Currency(Currency::from_scaled(0))),
        raw::Kind::Decimal { scale, .. } => Some(Value::Decimal(Decimal::new(0, scale).unwrap())),
        raw::Kind::DateTime | raw::Kind::Guid | raw::Kind::ShortText { .. } => None,
    }
}

fn definition(kind: raw::Kind, name: &str) -> RetainedFieldAdditionDefinition {
    let default = default_value(kind);
    let mut column = match kind {
        raw::Kind::Decimal { precision, scale } => {
            CreateColumn::decimal(name, DecimalConstraints::new(precision, scale).unwrap())
        }
        raw::Kind::ShortText { units, compression } => {
            CreateColumn::short_text(name, usize::from(units))
                .unwrap()
                .with_allow_zero_length(true)
                .unwrap()
                .with_unicode_compression(compression)
                .unwrap()
        }
        _ => CreateColumn::new(name, scalar(kind)),
    }
    .with_required(false);
    if let Some(value) = &default {
        column = column.with_default_literal(value).unwrap();
    }
    RetainedFieldAdditionDefinition::capture(column, &mut budget(), &|| false).unwrap()
}

#[test]
fn retained_typed_field_addition_matches_all_twelve_complete_images_and_old_row_values() {
    for case in fields::CASES {
        let source = fields::source(case);
        let old = read_table(
            &mut Cursor::new(&source.bytes),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        for kind in raw::KINDS {
            let definition = definition(kind, "AddedValue");
            let expected = raw::after(
                &source.bytes,
                source.definition,
                source.catalog_root,
                "AddedValue",
                kind,
            );
            let native = image(&source.bytes);
            let facts = native
                .inspect_retained_typed_field_addition_source(
                    "Table1",
                    definition.target(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert!(facts.maximum_name_utf16_units() >= "AddedValue".len());
            let candidate = native
                .prepare_retained_typed_field_addition(
                    "Table1",
                    &definition,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            equal_image(candidate.before_bytes(), &source.bytes);
            equal_image(candidate.after_bytes(), &expected);
            assert_eq!(candidate.native_number(), facts.native_number());
            assert_eq!(candidate.column_id(), facts.column_id());
            let new = read_table(
                &mut Cursor::new(candidate.after_bytes()),
                SCOPE,
                "Table1",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            for (old, new) in old.rows().iter().zip(new.rows()) {
                assert_eq!(&new[..old.len()], old);
                assert_eq!(
                    new.last(),
                    Some(&if matches!(kind, raw::Kind::Boolean) {
                        Value::Boolean(false)
                    } else {
                        Value::Null
                    })
                );
            }
        }
    }
}

#[test]
fn retained_typed_field_addition_fresh_inserts_use_exact_gallery_defaults() {
    let source = fields::source(fields::Case::FirstNumber);
    for kind in raw::KINDS {
        let definition = definition(kind, "AddedValue");
        let added = image(&source.bytes)
            .prepare_retained_typed_field_addition("Table1", &definition, &mut budget(), &|| false)
            .unwrap();
        let expected = raw::append_omitted(added.after_bytes(), source.definition, kind);
        let request = OmittedRowInsert::new(vec![
            InsertFieldValue::Omitted,
            InsertFieldValue::Omitted,
            InsertFieldValue::Omitted,
        ]);
        let inserted = image(added.after_bytes())
            .plan_retained_omitted_row_inserts("Table1", &[request], &mut budget(), &|| false)
            .unwrap();
        equal_image(inserted.after_bytes(), &expected);
        let rows = read_table(
            &mut Cursor::new(inserted.after_bytes()),
            SCOPE,
            "Table1",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows().last().unwrap().last(),
            Some(&default_value(kind).unwrap_or(Value::Null))
        );
    }
}

#[test]
fn retained_typed_field_addition_unnamed_advice_and_gallery_request_guards_are_distinct() {
    let before = fields::source(fields::Case::FirstNumber).bytes;
    let zero = Value::Currency(Currency::from_scaled(0));
    let unnamed = CreateColumn::new("", ScalarType::Currency)
        .with_required(false)
        .with_default_literal(&zero)
        .unwrap();
    let target = RetainedFieldAdditionType::capture(unnamed, &mut budget(), &|| false).unwrap();
    assert!(
        image(&before)
            .inspect_retained_typed_field_addition_source("Table1", target, &mut budget(), &|| {
                false
            })
            .unwrap()
            .maximum_name_utf16_units()
            > 0
    );
    assert!(matches!(
        RetainedFieldAdditionDefinition::capture(unnamed, &mut budget(), &|| false),
        Err(CreateDatabaseError::InvalidName)
    ));
    let duplicate = definition(raw::Kind::Currency, "Score");
    assert!(matches!(
        image(&before).prepare_retained_typed_field_addition(
            "Table1",
            &duplicate,
            &mut budget(),
            &|| false
        ),
        Err(CurrentFieldAdditionError::DuplicateName)
    ));
    for column in [
        CreateColumn::new("Amount", ScalarType::Currency).with_required(false),
        CreateColumn::new("Amount", ScalarType::Currency)
            .with_required(true)
            .with_default_literal(&zero)
            .unwrap(),
        CreateColumn::new("Amount", ScalarType::Currency)
            .with_required(false)
            .with_default_literal(&zero)
            .unwrap()
            .with_description("not gallery metadata")
            .unwrap(),
    ] {
        assert!(matches!(
            RetainedFieldAdditionDefinition::capture(column, &mut budget(), &|| false),
            Err(CreateDatabaseError::Declaration)
        ));
    }
    let definition = definition(raw::Kind::Currency, "Amount");
    assert!(matches!(
        image(&before).prepare_retained_typed_field_addition(
            "Table1",
            &definition,
            &mut budget(),
            &|| true
        ),
        Err(CurrentFieldAdditionError::Inspect(
            crate::InspectError::Cancelled
        ))
    ));
}

#[test]
fn retained_typed_field_addition_preserves_ascii_case_variant_dictionary_identity() {
    let source = fields::source(fields::Case::FirstNumber);
    let before = raw::lowercase_required(&source.bytes, source.catalog_root);
    let expected = raw::after(
        &before,
        source.definition,
        source.catalog_root,
        "Amount",
        raw::Kind::Currency,
    );
    let definition = definition(raw::Kind::Currency, "Amount");
    let candidate = image(&before)
        .prepare_retained_typed_field_addition("Table1", &definition, &mut budget(), &|| false)
        .unwrap();
    equal_image(candidate.after_bytes(), &expected);
}
