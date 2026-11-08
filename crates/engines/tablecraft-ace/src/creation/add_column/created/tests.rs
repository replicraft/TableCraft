//! Source and complete independent SDK-oracle checks precede candidate use.
use super::CreatedAddColumnError;
use crate::{
    AddColumnError, CreateColumn, CreateDatabaseError, FirstTableDefinition, InspectError,
    NativeImage, check_table, create_ace16_database, inspect_table_properties,
};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::{
    canonical_created_add_column as raw, canonical_initial_table_creation as initial,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(782),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn source_gate(bytes: &[u8], names: &[&str]) {
    let captured = image(bytes);
    let checked = check_table(
        &mut captured.reader(),
        captured.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 0);
    assert_eq!(checked.indexes().len(), 1);
    assert_eq!(checked.indexes()[0].index().name(), "PrimaryId");
    assert_eq!(checked.indexes()[0].index().root_page(), 9);
    assert_eq!(checked.schema().columns()[0].flags(), 7);
    assert_eq!(
        checked
            .schema()
            .columns()
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>(),
        names
    );
}

#[test]
fn sdk_inline_default_zero_source_and_after_are_exact_before_candidate_use() {
    let before = raw::default_zero_source();
    let expected = raw::append(&before, "Extra");
    source_gate(&before, &["Id", "Score"]);
    source_gate(&expected, &["Id", "Score", "Extra"]);
    assert_eq!(
        hash(&before),
        "6a17e6cf803d42e10a5bc7f240852ae44c69fd2b10d451e75379d9166aacd808"
    );
    assert_eq!(
        hash(&expected),
        "cc340fbd9f841d37169ab6e7b0be63e4704cbfc6f169fd59939f804a824e54d5"
    );
    let candidate = image(&before)
        .prepare_created_add_int32_column_candidate("items", "Extra", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.after_bytes(), expected);
    assert_eq!(candidate.table(), "Items");
    assert_eq!(candidate.column(), "Extra");
    assert_eq!(candidate.column_count(), 3);
    assert_eq!(candidate.source_profile(), 2);
    let columns = candidate
        .source_definition()
        .borrowed_columns(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(columns[1].default_literal(), Some(&Value::Int32(0)));
    let reopened = image(candidate.after_bytes());
    let properties = inspect_table_properties(
        &mut reopened.reader(),
        reopened.catalog().catalog().scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(properties.groups().len(), 1);
    assert_eq!(properties.groups()[0].name(), "Score");
    assert!(matches!(
        image(&before)
            .prepare_add_int32_column_candidate("Items", "Extra", &mut budget(), &|| false,),
        Err(AddColumnError::Source)
    ));
}

#[test]
fn generated_primary_scalar_sources_keep_old_fields_and_literal_index_tail() {
    for case in initial::CASES {
        let source = initial::source(case);
        let expected = raw::append(&source.bytes, "New 空 🦀");
        let candidate = image(&source.bytes)
            .prepare_created_add_int32_column_candidate(
                "Items",
                "New 空 🦀",
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(candidate.after_bytes(), expected, "{case:?}");
        assert_eq!(candidate.column_count(), source.columns.len() + 1);
        assert_eq!(
            &candidate.after_bytes()[9 * 4096..10 * 4096],
            &source.bytes[9 * 4096..10 * 4096]
        );
    }
}

#[test]
fn structural_names_optional_policies_and_constant_default_are_reconstructed() {
    let default = Value::Text("A \"quote\" 空".into());
    let columns = [
        CreateColumn::new("Number", ScalarType::Int32)
            .with_primary_key("Key 空")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::short_text("Label", 32)
            .unwrap()
            .with_required(false)
            .with_allow_zero_length(true)
            .unwrap()
            .with_default_literal(&default)
            .unwrap()
            .with_description("Description 空")
            .unwrap(),
    ];
    let definition =
        FirstTableDefinition::capture("Inventory 空", &columns, &mut budget(), &|| false).unwrap();
    let original =
        create_ace16_database(definition.table(), &columns, &[], &mut budget(), &|| false).unwrap();
    let candidate = image(original.as_bytes())
        .prepare_created_add_int32_column_candidate(
            "Inventory 空",
            "Extra",
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(candidate.table(), "Inventory 空");
    let retained = candidate
        .source_definition()
        .borrowed_columns(&mut budget(), &|| false)
        .unwrap();
    assert_eq!(retained[1].required(), Some(false));
    assert_eq!(retained[1].allow_zero_length(), Some(true));
    assert_eq!(retained[1].default_literal(), Some(&default));
    assert_eq!(retained[1].description(), Some("Description 空"));
    assert_eq!(
        &candidate.after_bytes()[..7 * 4096],
        &original.as_bytes()[..7 * 4096]
    );
}

#[test]
fn declaration_collision_and_complete_source_drift_refuse_separately() {
    let source = raw::default_zero_source();
    assert!(matches!(
        image(&source).prepare_created_add_int32_column_candidate(
            "Items",
            "score",
            &mut budget(),
            &|| false,
        ),
        Err(CreatedAddColumnError::Declaration(
            CreateDatabaseError::DuplicateColumn
        ))
    ));
    let mut drifted = source.clone();
    drifted[7 * 4096 + 4000] = 1;
    assert!(matches!(
        image(&drifted).prepare_created_add_int32_column_candidate(
            "Items",
            "Extra",
            &mut budget(),
            &|| false,
        ),
        Err(CreatedAddColumnError::Source)
    ));
    let mut extended = source;
    extended.extend_from_slice(&[0; 4096]);
    assert!(matches!(
        image(&extended).prepare_created_add_int32_column_candidate(
            "Items",
            "Extra",
            &mut budget(),
            &|| false,
        ),
        Err(CreatedAddColumnError::Source)
    ));
}

#[test]
fn cancellation_and_native_resource_limits_remain_typed() {
    let source = initial::source(initial::Case::LongInteger).bytes;
    assert!(matches!(
        image(&source).prepare_created_add_int32_column_candidate(
            "Items",
            "Extra",
            &mut budget(),
            &|| true,
        ),
        Err(CreatedAddColumnError::Inspect(InspectError::Cancelled))
    ));
    let mut exhausted = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        image(&source).prepare_created_add_int32_column_candidate(
            "Items",
            "Extra",
            &mut exhausted,
            &|| false,
        ),
        Err(CreatedAddColumnError::Inspect(_))
    ));
}

#[test]
fn sdk_mixed_scalar_source_hash_and_complete_after_are_independent() {
    let source = initial::source(initial::Case::Mixed);
    let expected = raw::append(&source.bytes, "Extra");
    source_gate(
        &source.bytes,
        &source
            .columns
            .iter()
            .map(|field| field.name)
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        hash(&source.bytes),
        "98f4c344e2a616a7b5b6fa73143fc92322fa61c5f80cd706f8e920e7f3ff6d47"
    );
    assert_eq!(
        hash(&expected),
        "426adf5f2e5ffab6392f665b84e3d0e4773314c197b98b7a53f5b79dd1cbf6ec"
    );
    let candidate = image(&source.bytes)
        .prepare_created_add_int32_column_candidate("Items", "Extra", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.after_bytes(), expected);
}

#[test]
fn fifteen_field_source_reaches_sixteen_without_expanding_the_profile_limit() {
    let mixed = initial::source(initial::Case::Mixed).bytes;
    let before = raw::append(&mixed, "Fifteenth");
    let expected = raw::append(&before, "Sixteenth");
    let candidate = image(&before)
        .prepare_created_add_int32_column_candidate("Items", "Sixteenth", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.after_bytes(), expected);
    assert_eq!(candidate.column_count(), 16);
    assert!(matches!(
        image(&expected).prepare_created_add_int32_column_candidate(
            "Items",
            "Seventeenth",
            &mut budget(),
            &|| false,
        ),
        Err(CreatedAddColumnError::Source)
    ));
}
