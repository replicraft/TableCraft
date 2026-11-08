//! Independent original/SDK expected SOURCE precedes the new candidate API.
use super::AddColumnError;
use crate::{CreateDatabaseError, InspectError, NativeImage, check_table, read_table};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_add_column, canonical_creation, canonical_native17_creation,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(781),
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

fn source_gate(source: &canonical_add_column::Source, name: &str) {
    for (bytes, names) in [
        (&source.before, vec!["Id", "Score"]),
        (&source.after, vec!["Id", "Score", name]),
    ] {
        let captured = image(bytes);
        let scope = captured.catalog().catalog().scope();
        let checked = check_table(
            &mut captured.reader(),
            scope,
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(checked.rows(), 0);
        assert!(checked.indexes().is_empty());
        assert_eq!(
            checked
                .schema()
                .columns()
                .iter()
                .map(|field| field.name())
                .collect::<Vec<_>>(),
            names,
        );
        assert!(
            read_table(
                &mut captured.reader(),
                scope,
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows()
            .is_empty()
        );
    }
}

#[test]
fn source_and_sdk_hashes_qualify_before_complete_candidate_comparison() {
    let source = canonical_add_column::source("Extra");
    source_gate(&source, "Extra");
    assert_eq!(
        hash(&source.before),
        "a75a6160bd6507c11fad247dae8e854e1eab83db262bfab258034921bcdc74d1"
    );
    assert_eq!(
        hash(&source.after),
        "ab982f44da01bd3725cce7b8d55acc6e67827ef43906b75ba951201dcf875010"
    );
    let candidate = image(&source.before)
        .prepare_add_int32_column_candidate("Items", "Extra", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.before_bytes(), source.before);
    assert_eq!(candidate.after_bytes(), source.after);
    assert_eq!(candidate.table(), "Items");
    assert_eq!(candidate.column(), "Extra");
    assert_eq!(candidate.column_count(), 3);
}

#[test]
fn unicode_column_names_use_the_same_source_roles_and_exact_independent_tail() {
    let source = canonical_add_column::source("New 空 🦀");
    source_gate(&source, "New 空 🦀");
    let candidate = image(&source.before)
        .prepare_add_int32_column_candidate("items", "New 空 🦀", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.after_bytes(), source.after);
    assert_eq!(candidate.table(), "Items");
}

#[test]
fn populated_sources_unknown_tail_bytes_and_duplicate_or_invalid_names_refuse() {
    let populated = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    assert!(matches!(
        image(&populated).prepare_add_int32_column_candidate(
            "Items",
            "Extra",
            &mut budget(),
            &|| false
        ),
        Err(AddColumnError::Source)
    ));
    let mut source = canonical_add_column::source("Extra").before;
    *source.last_mut().unwrap() = 71;
    assert!(matches!(
        image(&source)
            .prepare_add_int32_column_candidate("Items", "Extra", &mut budget(), &|| false),
        Err(AddColumnError::Source)
    ));
    let source = canonical_add_column::source("Extra").before;
    for name in ["", " invalid", "bad.name"] {
        assert!(matches!(
            image(&source).prepare_add_int32_column_candidate(
                "Items",
                name,
                &mut budget(),
                &|| false
            ),
            Err(AddColumnError::Declaration(
                CreateDatabaseError::InvalidName
            ))
        ));
    }
    assert!(matches!(
        image(&source)
            .prepare_add_int32_column_candidate("Items", "sCoRe", &mut budget(), &|| false),
        Err(AddColumnError::Declaration(
            CreateDatabaseError::DuplicateColumn
        ))
    ));
}

#[test]
fn cancellation_and_resource_exhaustion_publish_no_candidate() {
    let source = canonical_add_column::source("Extra").before;
    assert!(matches!(
        image(&source)
            .prepare_add_int32_column_candidate("Items", "Extra", &mut budget(), &|| true),
        Err(AddColumnError::Inspect(InspectError::Cancelled))
    ));
    let mut tiny = ResourceBudget::new(0, 0, 0);
    assert!(
        image(&source)
            .prepare_add_int32_column_candidate("Items", "Extra", &mut tiny, &|| false)
            .is_err()
    );
}
