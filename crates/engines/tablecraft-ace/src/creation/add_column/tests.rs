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

fn checked_empty_columns(bytes: &[u8], table: &str, names: &[&str]) {
    let captured = image(bytes);
    let scope = captured.catalog().catalog().scope();
    let checked = check_table(&mut captured.reader(), scope, table, &mut budget(), &|| {
        false
    })
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
        read_table(&mut captured.reader(), scope, table, &mut budget(), &|| {
            false
        })
        .unwrap()
        .rows()
        .is_empty()
    );
}

fn source_gate(source: &canonical_add_column::Source, name: &str) {
    checked_empty_columns(&source.before, "Items", &["Id", "Score"]);
    checked_empty_columns(&source.after, "Items", &["Id", "Score", name]);
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
fn decoded_user_names_and_the_sixteen_field_boundary_are_structural() {
    let minimum = canonical_add_column::one_column_source();
    let minimum_after = canonical_add_column::append(&minimum, "Score");
    checked_empty_columns(&minimum, "Items", &["Id"]);
    checked_empty_columns(&minimum_after, "Items", &["Id", "Score"]);
    assert_eq!(
        hash(&minimum),
        "d66026e33b8375e6106728acaa22daa2bff3c4c7ef11b59736023cec50215ed2"
    );
    assert_eq!(
        hash(&minimum_after),
        "a75a6160bd6507c11fad247dae8e854e1eab83db262bfab258034921bcdc74d1"
    );
    let candidate = image(&minimum)
        .prepare_add_int32_column_candidate("Items", "Score", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.after_bytes(), minimum_after);

    let raw = canonical_creation::source(canonical_creation::Case::Named);
    let mut before = canonical_native17_creation::source(raw.bytes);
    before[7 * 4096 + 16..7 * 4096 + 20].fill(0);
    let empty = canonical_add_column::source("Extra").before;
    before[8 * 4096..9 * 4096].copy_from_slice(&empty[8 * 4096..9 * 4096]);
    let expected = canonical_add_column::append(&before, "New column");
    checked_empty_columns(&before, "People Stats", &["Person Id", "Score Δ"]);
    checked_empty_columns(
        &expected,
        "People Stats",
        &["Person Id", "Score Δ", "New column"],
    );
    let candidate = image(&before)
        .prepare_add_int32_column_candidate("People Stats", "New column", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.table(), "People Stats");
    assert_eq!(candidate.after_bytes(), expected);

    let mut before = empty;
    let mut names = vec!["Id".to_owned(), "Score".to_owned()];
    for count in 3..=15 {
        let name = format!("C{count}");
        before = canonical_add_column::append(&before, &name);
        names.push(name);
    }
    let expected = canonical_add_column::append(&before, "Last");
    assert_eq!(
        hash(&before),
        "6f93aa2015d1b455b85e11ae3907851d1ccba2f4c139d7f3333090925b6756d7"
    );
    assert_eq!(
        hash(&expected),
        "009e7da6d0b46542aa313776e022829077987acfc242197cce4e31c6c98fba26"
    );
    checked_empty_columns(
        &before,
        "Items",
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    names.push("Last".into());
    checked_empty_columns(
        &expected,
        "Items",
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let candidate = image(&before)
        .prepare_add_int32_column_candidate("Items", "Last", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(candidate.column_count(), 16);
    assert_eq!(candidate.after_bytes(), expected);
    assert!(matches!(
        image(candidate.after_bytes()).prepare_add_int32_column_candidate(
            "Items",
            "Over limit",
            &mut budget(),
            &|| false
        ),
        Err(AddColumnError::Source)
    ));
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
