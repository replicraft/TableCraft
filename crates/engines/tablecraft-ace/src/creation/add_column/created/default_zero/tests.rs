//! Independent SDK full-image pins precede candidate and source-admission checks.
use super::CreatedDefaultZeroAddColumnError as Error;
use crate::{CreateColumn, NativeImage, create_ace16_database, inspect_table_properties};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::fixtures::access::canonical_created_default_zero as raw;

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 100_000, 200_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(783),
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

fn qualify(case: raw::Source, table: &str, column: &str, before_hash: &str, after_hash: &str) {
    let before = raw::source(case);
    let untouched = before.clone();
    let oracle = raw::append(&before, column);
    // Both complete images are pinned to public SDK ADD + fresh inert reopen.
    // The raw oracle never invokes a production encoder or candidate planner.
    assert_eq!(hash(&before), before_hash);
    assert_eq!(hash(&oracle), after_hash);
    assert_eq!(before.len(), 40960);
    assert_eq!(oracle.len(), 45056);
    let candidate = image(&before)
        .prepare_created_add_default_zero_int32_column_candidate(
            table,
            column,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(before, untouched);
    assert_eq!(candidate.before_bytes(), before);
    assert_eq!(candidate.after_bytes(), oracle);
    assert_eq!(candidate.table(), table);
    assert_eq!(candidate.column(), column);
    let reopened = image(candidate.after_bytes());
    let properties = inspect_table_properties(
        &mut reopened.reader(),
        reopened.catalog().catalog().scope(),
        table,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let group = properties
        .groups()
        .iter()
        .find(|group| group.name() == column)
        .unwrap();
    assert_eq!(group.properties().len(), 1);
    assert_eq!(group.properties()[0].name(), "DefaultValue");
    assert_eq!(group.properties()[0].raw_value(), [b'0', 0]);
}

#[test]
fn first_gui_long_default_zero_matches_complete_sdk_forwarded_catalog_after() {
    qualify(
        raw::Source::IdOnly,
        "Table1",
        "Score",
        "b52d124892cdaab8fbe9c5028c876f57f1f82ee280cea7984aad68f1ead5eb13",
        "d60f5ced66fdc200d62b24c7fcca4cb555731d670e8f29bd6ebb95f100509c88",
    );
}

#[test]
fn existing_inline_zero_matches_complete_sdk_external_property_after() {
    qualify(
        raw::Source::ExistingZero,
        "Items",
        "Extra",
        "6a17e6cf803d42e10a5bc7f240852ae44c69fd2b10d451e75379d9166aacd808",
        "1f3bb5bd37de1978587a233cc98da7a71b5f7262619180f1a2b548534f825bef",
    );
}

#[test]
fn old_no_default_profile_remains_same_size_and_default_absent() {
    let before = raw::source(raw::Source::IdOnly);
    let candidate = image(&before)
        .prepare_created_add_int32_column_candidate("Table1", "Score", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.after_bytes().len(), before.len());
    let reopened = image(candidate.after_bytes());
    let properties = inspect_table_properties(
        &mut reopened.reader(),
        reopened.catalog().catalog().scope(),
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(properties.raw_bytes().is_empty());
}

#[test]
fn existing_external_property_source_and_native_timestamp_refuse() {
    let source = raw::source(raw::Source::IdOnly);
    let external = raw::append(&source, "Score");
    assert!(
        image(&external)
            .prepare_created_add_default_zero_int32_column_candidate(
                "Table1",
                "Extra",
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
    let mut historical = source;
    // User catalog DateUpdate fixed payload; source reconstruction must reject.
    let root =
        u16::from_le_bytes(historical[4 * 4096 + 28..4 * 4096 + 30].try_into().unwrap()) as usize;
    historical[4 * 4096 + root + 24] = 1;
    assert!(
        image(&historical)
            .prepare_created_add_default_zero_int32_column_candidate(
                "Table1",
                "Score",
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
}

#[test]
fn unqualified_property_dictionary_is_not_reordered_or_guessed() {
    let columns = [
        CreateColumn::new("ID", ScalarType::Int32)
            .with_primary_key("PrimaryKey")
            .unwrap()
            .with_incrementing_autonumber()
            .unwrap(),
        CreateColumn::new("Score", ScalarType::Int32)
            .with_description("Preserved text")
            .unwrap(),
    ];
    let before = create_ace16_database("Table1", &columns, &[], &mut budget(), &|| false).unwrap();
    assert!(matches!(
        image(before.as_bytes()).prepare_created_add_default_zero_int32_column_candidate(
            "Table1",
            "Extra",
            &mut budget(),
            &|| false,
        ),
        Err(Error::Source)
    ));
}

#[test]
fn proposed_name_validation_and_cancellation_do_not_publish_candidate() {
    let before = raw::source(raw::Source::IdOnly);
    assert!(
        image(&before)
            .prepare_created_add_default_zero_int32_column_candidate(
                "Table1",
                "ID",
                &mut budget(),
                &|| false,
            )
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_created_add_default_zero_int32_column_candidate(
                "Table1",
                "Score",
                &mut budget(),
                &|| true,
            )
            .is_err()
    );
    assert!(
        image(&before)
            .prepare_created_add_default_zero_int32_column_candidate(
                "Table1",
                "Score",
                &mut ResourceBudget::new(0, 0, 0),
                &|| false,
            )
            .is_err()
    );
    assert_eq!(
        hash(&before),
        "b52d124892cdaab8fbe9c5028c876f57f1f82ee280cea7984aad68f1ead5eb13"
    );
}
