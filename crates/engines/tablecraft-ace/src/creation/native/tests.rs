#[cfg(feature = "journal")]
use crate::{AddTableSourceProfile, EditError, RowInsert, WriteBlocker};
use crate::{
    CreateColumn, CreateDatabaseError, FirstTableDefinition, FirstTableError, NativeImage,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
#[cfg(feature = "journal")]
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields;
use tablecraft_testkit::fixtures::access::canonical_wide_table_creation as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(256 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(7301),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn declarations<'a>(names: &'a [String], zero: &'a Value) -> Vec<CreateColumn<'a>> {
    names
        .iter()
        .enumerate()
        .map(|(position, name)| {
            let column = CreateColumn::new(name, ScalarType::Int32);
            if position == 0 {
                column
                    .with_primary_key("PrimaryKey")
                    .unwrap()
                    .with_incrementing_autonumber()
                    .unwrap()
            } else {
                column
                    .with_required(false)
                    .with_default_literal(zero)
                    .unwrap()
            }
        })
        .collect()
}

#[test]
fn native_creation_matches_independent_255_column_definition_and_external_properties() {
    let source = raw::source();
    let names = (0..255).map(raw::field_name).collect::<Vec<_>>();
    let zero = Value::Int32(0);
    let columns = declarations(&names, &zero);
    assert!(matches!(
        FirstTableDefinition::capture("Table1", &columns, &mut budget(), &|| false),
        Err(CreateDatabaseError::ColumnLimit)
    ));
    let definition =
        FirstTableDefinition::capture_native("Table1", &columns, &mut budget(), &|| false).unwrap();
    assert!(definition.is_native());
    let plan = capture(&source.before)
        .plan_first_table_native(definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.after_bytes().len(), source.after.len());
    assert_eq!(
        plan.after_bytes()
            .iter()
            .zip(&source.after)
            .position(|(left, right)| left != right),
        None
    );
    assert_eq!(plan.before_bytes(), source.before);
}

#[test]
fn original_planner_rejects_native_request_even_when_its_column_count_fits_original() {
    let source = raw::source();
    let columns = [CreateColumn::new("ID", ScalarType::Int32)
        .with_primary_key("PrimaryKey")
        .unwrap()
        .with_incrementing_autonumber()
        .unwrap()];
    let definition =
        FirstTableDefinition::capture_native("Table1", &columns, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        capture(&source.before).plan_first_table(definition, &mut budget(), &|| false),
        Err(FirstTableError::Create(CreateDatabaseError::Declaration))
    ));
}

#[cfg(feature = "journal")]
#[test]
fn native_count_append_preserves_packed_and_retained_source_cohorts_and_complete_images() {
    let initial = raw::source();
    let retained = fields::source(fields::Case::FirstNumber);
    let names = (0..255).map(raw::field_name).collect::<Vec<_>>();
    let zero = Value::Int32(0);
    let columns = declarations(&names, &zero);
    for (before, profile) in [
        (initial.after.as_slice(), AddTableSourceProfile::Mixed),
        (retained.bytes.as_slice(), AddTableSourceProfile::Retained),
    ] {
        let expected = raw::appended::source(before, "Wide");
        let definition =
            FirstTableDefinition::capture_native("Wide", &columns, &mut budget(), &|| false)
                .unwrap();
        let plan = capture(before)
            .plan_add_table_native(definition, &mut budget(), &|| false)
            .unwrap();
        assert!(plan.is_native());
        assert_eq!(plan.source_profile(), profile);
        assert_eq!(plan.after_bytes().len(), expected.after.len());
        assert_eq!(
            plan.after_bytes()
                .iter()
                .zip(&expected.after)
                .position(|(left, right)| left != right),
            None
        );
    }
}

#[cfg(feature = "journal")]
#[test]
fn empty_fixed_wide_metadata_has_no_candidate_when_a_complete_row_cannot_fit() {
    let source = raw::source();
    let names = (0..255).map(raw::field_name).collect::<Vec<_>>();
    let columns = names
        .iter()
        .map(|name| CreateColumn::new(name, ScalarType::Guid))
        .collect::<Vec<_>>();
    let definition =
        FirstTableDefinition::capture_manual_native("Wide", &columns, &mut budget(), &|| false)
            .unwrap();
    let plan = capture(&source.before)
        .plan_first_table_native(definition, &mut budget(), &|| false)
        .unwrap();
    let after = plan.after_bytes();
    // 255 fixed GUIDs own 4080 payload bytes. A complete logical count and
    // 32-byte mask require 4114 bytes, exceeding the real one-page DATA space.
    // Creation is still valid empty metadata; it invents no fitting-page hint.
    assert_eq!(&after[7 * 4096 + 59..7 * 4096 + 63], &[7, 3, 0, 0]);
    let maps = &after[3 * 4096..4 * 4096];
    let start = u16::from_le_bytes(maps[28..30].try_into().unwrap()) as usize;
    let end = u16::from_le_bytes(maps[26..28].try_into().unwrap()) as usize;
    assert_eq!(&maps[start..end], &[0; 13]);
    let rows = crate::read_table(
        &mut Cursor::new(after),
        CatalogScope::new(7301),
        "Wide",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.schema().columns().len(), 255);
    assert!(rows.rows().is_empty());
    let empty = capture(after)
        .plan_ordinary_complete_row_inserts("Wide", &[], &mut budget(), &|| false)
        .unwrap();
    assert_eq!(empty.after_bytes(), after);
    let request = RowInsert::new((0..255).map(|_| Value::Null).collect());
    assert!(matches!(
        capture(after).plan_ordinary_complete_row_inserts(
            "Wide",
            &[request],
            &mut budget(),
            &|| false
        ),
        Err(EditError::Unsupported(WriteBlocker::RowLayout))
    ));
}

#[test]
fn native_creation_crosses_property_map_windows_with_full_names_and_properties() {
    let source = raw::property_windows::source();
    let mut names = vec![String::from("ID")];
    names.extend((1..255).map(raw::property_windows::field_name));
    let default = Value::Text("d".repeat(253).into());
    let description = raw::property_windows::description();
    let columns = names
        .iter()
        .enumerate()
        .map(|(position, name)| {
            if position == 0 {
                CreateColumn::new(name, ScalarType::Int32)
                    .with_primary_key("PrimaryKey")
                    .unwrap()
                    .with_incrementing_autonumber()
                    .unwrap()
            } else {
                CreateColumn::short_text(name, 255)
                    .unwrap()
                    .with_required(false)
                    .with_allow_zero_length(true)
                    .unwrap()
                    .with_default_literal(&default)
                    .unwrap()
                    .with_description(&description)
                    .unwrap()
            }
        })
        .collect::<Vec<_>>();
    // Original request storage remains inline even for a count it admits.
    assert!(matches!(
        FirstTableDefinition::capture("Table1", &columns[..16], &mut budget(), &|| false),
        Err(CreateDatabaseError::PageCapacity)
    ));
    let definition =
        FirstTableDefinition::capture_native("Table1", &columns, &mut budget(), &|| false).unwrap();
    let plan = capture(&source.before)
        .plan_first_table_native(definition, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(source.property_pages.len(), 75);
    assert_eq!(plan.after_bytes().len(), source.after.len());
    assert_eq!(
        plan.after_bytes()
            .iter()
            .zip(&source.after)
            .position(|(left, right)| left != right),
        None
    );
    let properties = crate::inspect_table_properties(
        &mut Cursor::new(plan.after_bytes()),
        CatalogScope::new(7301),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.raw_bytes(), source.property_body);
    assert_eq!(properties.groups().len(), 254);
}

#[test]
fn native_active_column_error_reports_255_before_allocating_owned_fields() {
    let names = (0..256)
        .map(|index| format!("Field{index}"))
        .collect::<Vec<_>>();
    let columns = names
        .iter()
        .map(|name| CreateColumn::new(name, ScalarType::Int32))
        .collect::<Vec<_>>();
    let mut no_allocation = ResourceBudget::new(0, 100_000, 100_000_000);
    let error =
        FirstTableDefinition::capture_manual_native("Wide", &columns, &mut no_allocation, &|| {
            false
        })
        .err()
        .expect("256 active fields exceed the native limit");
    assert!(matches!(error, CreateDatabaseError::NativeColumnLimit));
    assert_eq!(
        error.to_string(),
        "native creation requires between one and 255 scalar columns"
    );
}
