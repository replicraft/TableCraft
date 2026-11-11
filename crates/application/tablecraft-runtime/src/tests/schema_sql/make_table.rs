//! Issued SQL creates its complete materialized relation through one native save.
mod context;
use super::{inspect, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, QueryOperation, Response, SaveOutcome, SaveSchemaChange, SaveWorker};
use std::fs;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{fields, populated};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn make_table_reviews_materialized_rows_and_saves_schema_plus_rows_without_sql_reexecution() {
    for (literals, sql, selected) in [
        (
            vec![],
            "SELECT CopiedScore INTO Archive FROM Items ORDER BY CopiedScore",
            vec![],
        ),
        (
            std::iter::once(None).chain((1..900).map(Some)).collect(),
            "SELECT CopiedScore INTO Archive FROM Items ORDER BY CopiedScore",
            std::iter::once(None).chain((1..900).map(Some)).collect(),
        ),
        (
            vec![None, Some(2), Some(5), Some(5), Some(9), Some(10)],
            "SELECT TOP 1 CopiedScore INTO Archive FROM Items WHERE CopiedScore >= 5 ORDER BY CopiedScore",
            vec![Some(5), Some(5)],
        ),
        (
            vec![Some(5)],
            "SELECT CopiedScore INTO Archive FROM Items WHERE CopiedScore > 100",
            vec![],
        ),
    ] {
        let seed = fields::source(fields::Case::FirstNumber).bytes;
        let before = populated::numbers(&seed, "Items", &literals);
        let expected = populated::numbers(&before, "Archive", &selected);
        let (_directory, path, worker, revision) = opened(&before);
        let target = inspect(&worker, revision, sql);
        assert_eq!(target.operation(), QueryOperation::Schema);
        let Response::TableCreationReviewed { capability } = review(&worker, &target).unwrap()
        else {
            panic!("expected populated creation review");
        };
        let witness = capability.reopen_witness();
        assert_eq!(witness.draft_revision(), None);
        assert_eq!(witness.inserted_rows(), Some(selected.len()));
        assert_eq!(witness.column_count(), 1);
        assert_image(witness.expected_bytes(), &expected);
        assert_image(&fs::read(&path).unwrap(), &before);
        let bundle = crate::journal_path(&path);
        assert!(!bundle.exists());
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_table_creation(capability, bundle.clone())
            .unwrap();
        let report = terminal(&saves);
        assert_eq!(report.id, id);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.matched_rows.is_none() && receipt.changed_rows == Some(selected.len())
            && receipt.schema_change == Some(SaveSchemaChange::TableAdded { column_count: 1 })));
        assert!(saves.acknowledge(id));
        assert_image(&fs::read(&path).unwrap(), &expected);
        let Response::Opened {
            document_revision: fresh,
            tables,
            ..
        } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("expected guarded populated-table reopen");
        };
        assert!(fresh > revision);
        assert!(tables.iter().any(|table| table.name == "Archive"));
        assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
        assert!(!bundle.exists());
        assert_image(&fs::read(path).unwrap(), &expected);
    }
}

#[test]
fn make_table_rejects_existing_destinations_closed_shapes_and_stale_inspections_without_writes() {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let before = populated::numbers(&seed, "Items", &[Some(1)]);
    let (_directory, path, worker, revision) = opened(&before);
    for sql in [
        "SELECT CopiedScore INTO Items FROM Items",
        "SELECT CopiedScore+1 AS Computed INTO Archive FROM Items",
        "SELECT CopiedScore INTO Archive FROM Items GROUP BY CopiedScore",
    ] {
        let target = inspect(&worker, revision, sql);
        assert!(review(&worker, &target).is_err());
        assert_image(&fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
    }
    let target = inspect(
        &worker,
        revision,
        "SELECT CopiedScore INTO Archive FROM Items",
    );
    let (_other_directory, _other_path, other_worker, _) = opened(&before);
    assert!(matches!(
        review(&other_worker, &target),
        Err(Error::StaleQuery)
    ));
    assert_image(&fs::read(path).unwrap(), &before);
}
