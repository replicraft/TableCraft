//! Inert source facts, without a lease, DROP candidate or save capability.
mod service;

use super::{request, stored::opened};
use crate::{Command, Error, FieldDropSourceInspection, Response};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::{
    self, after_off::deleted_field as raw,
};

fn inspect(
    worker: &crate::Worker,
    revision: u64,
    column: &str,
    native_number: usize,
) -> Result<FieldDropSourceInspection, Error> {
    let response = request(
        worker,
        Command::InspectFieldDropSource {
            document_revision: revision,
            table: "Table1".into(),
            column: column.into(),
            native_number,
        },
    )?;
    let Response::FieldDropSourceInspected { inspection } = response else {
        panic!("expected exact DROP source facts");
    };
    Ok(inspection)
}

#[test]
fn both_real_sources_publish_exact_metadata_and_take_no_exclusive_lease() {
    for (case, column) in [
        (raw::SourceCase::NumberHistory, "Score"),
        (raw::SourceCase::MixedTextHistory, "Label"),
    ] {
        let before = raw::source(case);
        let (_directory, path, worker, revision) = opened(&before);
        let facts = inspect(&worker, revision, column, 1).unwrap();
        assert_eq!(facts.document_revision, revision);
        assert_eq!(facts.table, "Table1");
        assert_eq!(facts.column, column);
        assert_eq!(facts.native_number, 1);
        assert!(facts.source_eligible);
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

#[test]
fn protected_and_unqualified_sources_refuse_without_stale_target_coercion() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let (_directory, path, worker, revision) = opened(&before);
    let protected = inspect(&worker, revision, "ID", 0).unwrap();
    assert!(!protected.source_eligible);
    assert_eq!(protected.column, "ID");
    assert_eq!(protected.native_number, 0);
    assert!(matches!(
        inspect(&worker, revision + 1, "Score", 1),
        Err(Error::StaleDocument)
    ));
    for (column, number) in [("score", 1), ("Score", 17), ("Friendly", 1)] {
        assert!(matches!(
            inspect(&worker, revision, column, number),
            Err(Error::StaleFieldDropTarget)
        ));
    }
    assert_eq!(std::fs::read(path).unwrap(), before);

    // The metadata exists, but this source has no explicit database OFF role.
    let unqualified = mixed::source(mixed::SourceCase::InlineNumber);
    let (_directory, path, worker, revision) = opened(&unqualified);
    assert!(
        !inspect(&worker, revision, "Score", 1)
            .unwrap()
            .source_eligible
    );
    assert_eq!(std::fs::read(path).unwrap(), unqualified);
}

#[test]
fn cancelled_inspection_publishes_no_facts_or_writes() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let (_directory, path, _worker, revision) = opened(&before);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    assert!(matches!(
        document.inspect_field_drop_source(revision, "Table1", "Score", 1, &|| true),
        Err(Error::Cancelled)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}
