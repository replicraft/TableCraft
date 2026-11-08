//! Manual blank-to-table requests retain explicit key absence and position.
use super::FirstTableDefinition;
#[cfg(feature = "journal")]
use crate::{AddTableError, RowInsert, inspect_table_schema, read_table};
use crate::{CreateColumn, CreateDatabaseError, InspectError, NativeImage};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
#[cfg(feature = "journal")]
use tablecraft_core::values::Currency;
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, canonical_creation, canonical_creation_primary,
    canonical_native17_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(499);

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn manual_keyless_and_primary_plans_match_independent_empty_source_images() {
    for keyed in [false, true] {
        let id = CreateColumn::new("Id", ScalarType::Int32);
        let id = if keyed {
            id.with_primary_key("PrimaryId").unwrap()
        } else {
            id
        };
        let columns = [id, CreateColumn::new("Score", ScalarType::Int32)];
        assert!(
            FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).is_err()
        );
        let definition =
            FirstTableDefinition::capture_manual("Items", &columns, &mut budget(), &|| false)
                .unwrap();
        assert!(definition.is_manual());
        assert_eq!(definition.primary_name(), keyed.then_some("PrimaryId"));
        assert_eq!(definition.primary_position(), keyed.then_some(0));
        let expected = if keyed {
            canonical_creation_primary::source(canonical_creation_primary::Case::Empty).bytes
        } else {
            canonical_native17_creation::source(
                canonical_creation::source(canonical_creation::Case::Empty).bytes,
            )
        };
        let plan = image(&canonical_blank_creation::source())
            .plan_first_table(definition, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.after_bytes(), expected);
        assert_eq!(plan.after_bytes().len(), if keyed { 10 } else { 9 } * 4096);
    }
}

#[test]
fn manual_capture_refuses_generated_fields_multiple_keys_empty_schema_and_cancellation() {
    let generated = CreateColumn::new("Id", ScalarType::Int32)
        .with_primary_key("PrimaryId")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap();
    assert!(matches!(
        FirstTableDefinition::capture_manual("Items", &[generated], &mut budget(), &|| false),
        Err(CreateDatabaseError::Declaration)
    ));
    let columns = [
        CreateColumn::new("First", ScalarType::Int32)
            .with_primary_key("FirstKey")
            .unwrap(),
        CreateColumn::new("Second", ScalarType::Int64)
            .with_primary_key("SecondKey")
            .unwrap(),
    ];
    assert!(matches!(
        FirstTableDefinition::capture_manual("Items", &columns, &mut budget(), &|| false),
        Err(CreateDatabaseError::Declaration)
    ));
    assert!(matches!(
        FirstTableDefinition::capture_manual("Items", &[], &mut budget(), &|| false),
        Err(CreateDatabaseError::ColumnLimit)
    ));
    assert!(matches!(
        FirstTableDefinition::capture_manual("Items", &columns[..1], &mut budget(), &|| true),
        Err(CreateDatabaseError::Inspect(InspectError::Cancelled))
    ));
}

#[cfg(feature = "journal")]
#[test]
fn manual_keys_keep_actual_position_and_existing_complete_first_insert_route() {
    for scalar in [ScalarType::Int32, ScalarType::Int64, ScalarType::Currency] {
        let key = match scalar {
            ScalarType::Int32 => Value::Int32(7),
            ScalarType::Int64 => Value::Int64(7),
            ScalarType::Currency => Value::Currency(Currency::from_scaled(7)),
            _ => unreachable!(),
        };
        for keyed in [false, true] {
            let column = CreateColumn::new("ActualKey", scalar);
            let column = if keyed {
                column.with_primary_key("ActualPrimary").unwrap()
            } else {
                column
            };
            let columns = [CreateColumn::new("First", ScalarType::Int32), column];
            let definition =
                FirstTableDefinition::capture_manual("Items", &columns, &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(definition.primary_position(), keyed.then_some(1));
            let borrowed = definition
                .borrowed_columns(&mut budget(), &|| false)
                .unwrap();
            assert!(
                borrowed
                    .iter()
                    .all(|column| !column.incrementing_autonumber())
            );
            assert_eq!(borrowed[0].primary_key_name(), None);
            assert_eq!(
                borrowed[1].primary_key_name(),
                keyed.then_some("ActualPrimary")
            );
            drop(borrowed);
            let plan = image(&canonical_blank_creation::source())
                .plan_first_table(definition, &mut budget(), &|| false)
                .unwrap();
            let captured = image(plan.after_bytes());
            let schema = inspect_table_schema(
                &mut captured.reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                schema
                    .columns()
                    .iter()
                    .all(|column| column.flags() & 4 == 0)
            );
            let expected = vec![Value::Int32(17), key.clone()];
            let inserted = captured
                .plan_ordinary_row_inserts(
                    "Items",
                    &[RowInsert::new(expected.clone())],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let rows = read_table(
                &mut image(inserted.after_bytes()).reader(),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), &[expected]);
        }
    }
}

#[cfg(feature = "journal")]
#[test]
fn manual_append_is_refused_before_entering_increment_only_role_construction() {
    let source = canonical_creation_primary::source(canonical_creation_primary::Case::Empty);
    let columns = [CreateColumn::new("Value", ScalarType::Int32)];
    let definition =
        FirstTableDefinition::capture_manual("Other", &columns, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        image(&source.bytes).plan_add_table(definition, &mut budget(), &|| false),
        Err(AddTableError::Create(CreateDatabaseError::Declaration))
    ));
}

#[test]
fn manual_owned_definition_keeps_default_policy_and_description_after_inputs_drop() {
    let definition = {
        let literal = Value::Text("  Guest  ".into());
        let description = String::from("Now() is descriptive 日本語");
        let columns = [
            CreateColumn::short_text("Label", 30)
                .unwrap()
                .with_required(false)
                .with_allow_zero_length(true)
                .unwrap()
                .with_default_literal(&literal)
                .unwrap()
                .with_description(&description)
                .unwrap(),
            CreateColumn::new("Key", ScalarType::Int64)
                .with_primary_key("ActualPrimary")
                .unwrap()
                .with_description("")
                .unwrap(),
        ];
        FirstTableDefinition::capture_manual("Items", &columns, &mut budget(), &|| false).unwrap()
    };
    let columns = definition
        .borrowed_columns(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(columns[0].required(), Some(false));
    assert_eq!(columns[0].allow_zero_length(), Some(true));
    assert_eq!(
        columns[0].default_literal(),
        Some(&Value::Text("  Guest  ".into()))
    );
    assert_eq!(
        columns[0].description(),
        Some("Now() is descriptive 日本語")
    );
    assert_eq!(columns[1].description(), Some(""));
    assert_eq!(columns[0].primary_key_name(), None);
    assert_eq!(columns[1].primary_key_name(), Some("ActualPrimary"));
}
