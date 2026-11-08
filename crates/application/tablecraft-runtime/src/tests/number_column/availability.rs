//! Availability is exact source/revision qualification, never empty-row inference.
use super::{open, request};
use crate::{Command, Error, NumberColumnAvailability, Response};
use tablecraft_testkit::fixtures::access::{canonical_created_number as raw, canonical_creation};

fn inspect(worker: &crate::Worker, revision: u64, table: &str) -> NumberColumnAvailability {
    let Response::NumberColumnAvailability { availability } = request(
        worker,
        Command::InspectNumberColumnAvailability {
            document_revision: revision,
            table: table.into(),
        },
    )
    .unwrap() else {
        panic!("expected revision-bound complete Number source inspection");
    };
    assert_eq!(availability.document_revision, revision);
    assert_eq!(availability.table, table);
    availability
}

#[test]
fn all_three_qualified_sources_produce_positive_facts_without_writes_or_lease() {
    for (case, table) in [
        (raw::Source::IdOnly, "Table1"),
        (raw::Source::GuiRequiredZero, "Table1"),
        (raw::Source::DefaultOnly, "Items"),
    ] {
        let before = raw::source(case);
        let (_directory, path, worker, revision) = crate::tests::stored::opened(&before);
        assert!(inspect(&worker, revision, table).available);
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn empty_external_property_and_keyless_sources_never_inherit_availability() {
    let before = raw::source(raw::Source::IdOnly);
    let external = raw::append(&before, raw::Source::IdOnly, "Score");
    let keyless = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    for (before, table) in [(external, "Table1"), (keyless, "Items")] {
        let (_directory, path, worker, revision) = crate::tests::stored::opened(&before);
        assert!(!inspect(&worker, revision, table).available);
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn replaced_document_rejects_the_old_inspection_revision_before_source_work() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, worker, revision) = crate::tests::stored::opened(&before);
    assert!(inspect(&worker, revision, "Table1").available);
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::InspectNumberColumnAvailability {
                document_revision: revision,
                table: "Table1".into(),
            }
        ),
        Err(Error::StaleDocument)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
