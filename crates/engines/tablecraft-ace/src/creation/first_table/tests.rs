use super::{FirstTableDefinition, FirstTableError};
use crate::{CreateColumn, NativeImage};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::{DecimalConstraints, ScalarType};
use tablecraft_testkit::fixtures::access::{
    canonical_blank_creation, canonical_initial_table_creation as fixtures,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(481),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn columns(raw: &[fixtures::Column]) -> Vec<CreateColumn<'_>> {
    raw.iter()
        .enumerate()
        .map(|(position, column)| {
            let scalar = match column.native_type {
                1 => ScalarType::Boolean,
                2 => ScalarType::Byte,
                3 => ScalarType::Int16,
                4 => ScalarType::Int32,
                5 => ScalarType::Currency,
                6 => ScalarType::Single,
                7 => ScalarType::Double,
                8 => ScalarType::DateTime,
                15 => ScalarType::Guid,
                19 => ScalarType::Int64,
                9 => return CreateColumn::short_binary(column.name, column.width.into()).unwrap(),
                10 => {
                    return CreateColumn::short_text(column.name, (column.width / 2).into())
                        .unwrap();
                }
                16 => {
                    let constraints =
                        DecimalConstraints::new(column.precision, column.scale).unwrap();
                    return CreateColumn::decimal(column.name, constraints);
                }
                _ => panic!("closed raw fixture declaration"),
            };
            let declaration = CreateColumn::new(column.name, scalar);
            if position == 0 {
                declaration
                    .with_primary_key("PrimaryId")
                    .unwrap()
                    .with_incrementing_autonumber()
                    .unwrap()
            } else {
                declaration
            }
        })
        .collect()
}

#[test]
fn canonical_blank_initialization_matches_each_independent_complete_schema() {
    let before = canonical_blank_creation::source();
    for case in fixtures::CASES {
        let expected = fixtures::source(case);
        let columns = columns(expected.columns);
        let definition =
            FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
        let plan = image(&before)
            .plan_first_table(definition, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), expected.bytes);
        assert_eq!(plan.table(), "Items");
        assert_eq!(plan.column_count(), expected.columns.len());

        let definition = plan.into_definition();
        assert_eq!(definition.table(), "Items");
        assert_eq!(definition.primary_name(), "PrimaryId");
        assert_eq!(definition.column_count(), expected.columns.len());
    }
}

#[test]
fn readable_noncanonical_bytes_and_existing_user_tables_refuse_initialization() {
    let mut changed = canonical_blank_creation::source();
    *changed.last_mut().unwrap() = 17;
    let existing = fixtures::source(fixtures::Case::IdOnly);
    let columns = columns(existing.columns);
    for bytes in [&changed, &existing.bytes] {
        let definition =
            FirstTableDefinition::capture("Items", &columns, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            image(bytes).plan_first_table(definition, &mut budget(), &|| false),
            Err(FirstTableError::NotCanonicalBlank)
        ));
    }
}

#[test]
fn original_definition_capture_requires_a_generated_primary_and_is_cancellable() {
    let plain = [CreateColumn::new("ID", ScalarType::Int32)];
    assert!(FirstTableDefinition::capture("Items", &plain, &mut budget(), &|| false).is_err());
    let generated = [plain[0].with_incrementing_autonumber().unwrap()];
    assert!(FirstTableDefinition::capture("Items", &generated, &mut budget(), &|| false).is_err());
    let generated = [generated[0].with_primary_key("PrimaryKey").unwrap()];
    assert!(FirstTableDefinition::capture("Items", &generated, &mut budget(), &|| true).is_err());
}
