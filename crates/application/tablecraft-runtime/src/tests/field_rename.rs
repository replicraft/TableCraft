//! Actual read-session qualification, with no hypothetical replacement or save.
use super::{request, stored::opened};
use crate::{Command, Error, FieldRenameSourceInspection, Response};
use tablecraft_testkit::fixtures::access::canonical_created_number::{
    Source, mixed::after_off as raw, repeated,
};

fn inspect(
    worker: &crate::Worker,
    revision: u64,
    column: &str,
    number: usize,
) -> Result<FieldRenameSourceInspection, Error> {
    match request(
        worker,
        Command::InspectFieldRenameSource {
            document_revision: revision,
            table: "Table1".into(),
            column: column.into(),
            native_number: number,
        },
    )? {
        Response::FieldRenameSourceInspected { inspection } => Ok(inspection),
        _ => panic!("expected exact field-source facts"),
    }
}

#[test]
fn real_number_and_text_source_facts_preserve_originals_and_take_no_lease() {
    for (case, column) in [
        (raw::SourceCase::Number, "Score"),
        (raw::SourceCase::Mixed, "Label"),
    ] {
        let before = raw::source(case);
        let (_directory, path, worker, revision) = opened(&before);
        let facts = inspect(&worker, revision, column, 1).unwrap();
        assert_eq!(facts.document_revision, revision);
        assert_eq!(facts.table, "Table1");
        assert_eq!(facts.column, column);
        assert_eq!(facts.native_number, 1);
        assert!(facts.source_eligible);
        assert_eq!(facts.same_width_ascii_utf16_units, Some(5));
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
fn primary_and_missing_off_profiles_are_unavailable_but_keep_true_identity() {
    for (before, field, number) in [
        (raw::source(raw::SourceCase::Number), "ID", 0),
        (repeated::source(Source::IdOnly), "Score", 1),
    ] {
        let (_directory, path, worker, revision) = opened(&before);
        let facts = inspect(&worker, revision, field, number).unwrap();
        assert_eq!(facts.column, field);
        assert_eq!(facts.native_number, number);
        assert!(!facts.source_eligible);
        assert_eq!(facts.same_width_ascii_utf16_units, None);
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

#[test]
fn stale_revision_true_name_and_native_number_never_publish_facts() {
    let before = raw::source(raw::SourceCase::Number);
    let (_directory, path, worker, revision) = opened(&before);
    assert!(matches!(
        inspect(&worker, revision + 1, "Score", 1),
        Err(Error::StaleDocument)
    ));
    for (column, number) in [("score", 1), ("Score", 0), ("Score", 99)] {
        assert!(matches!(
            inspect(&worker, revision, column, number),
            Err(Error::StaleFieldRenameTarget)
        ));
    }
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn cancelled_inspection_keeps_the_source_and_publishes_no_facts() {
    let before = raw::source(raw::SourceCase::Number);
    let (_directory, path, _worker, revision) = opened(&before);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), revision, &|| false).unwrap();
    assert!(matches!(
        document.inspect_field_rename_source(revision, "Table1", "Score", 1, &|| true),
        Err(Error::Cancelled)
    ));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn published_native_number_comes_from_bound_schema_not_display_position() {
    let mut bytes = repeated::source(Source::IdOnly);
    bytes[7 * 4096 + 41..7 * 4096 + 43].copy_from_slice(&18_u16.to_le_bytes());
    bytes[7 * 4096 + 105..7 * 4096 + 107].copy_from_slice(&17_u16.to_le_bytes());
    let (_directory, _path, worker, _) = opened(&bytes);
    let Response::Data { columns, .. } = request(
        &worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected actual native page metadata");
    };
    assert_eq!(columns[0].native_number, Some(0));
    assert_eq!(columns[1].name, "Score");
    assert_eq!(columns[1].native_number, Some(17));
}
