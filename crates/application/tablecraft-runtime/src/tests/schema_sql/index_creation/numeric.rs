//! Actual SQL review/save publishes numeric indexes through the shared schema service.
use super::super::{inspect, review};
use super::assert_image;
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveSchemaChange, SaveWorker};
use std::fs;
use tablecraft_testkit::fixtures::access::secondary_index::numeric::{
    self as raw, Kind, Population,
};

#[test]
fn numeric_index_sql_saves_complete_independent_images_and_reopens_only_its_source() {
    for kind in raw::KINDS.into_iter().filter(|kind| *kind != Kind::Int32) {
        for descending in [false, true] {
            let source = raw::source(kind, Population::NullableDuplicate, descending);
            let (_directory, path, worker, revision) = opened(&source.before);
            let sql = format!(
                "CREATE INDEX [{}] ON [{}] ([{}] {})",
                source.index_name,
                source.table,
                source.column,
                if descending { "DESC" } else { "ASC" },
            );
            let inspected = inspect(&worker, revision, &sql);
            let Response::IndexCreationReviewed { capability } =
                review(&worker, &inspected).unwrap()
            else {
                panic!("expected numeric index review for {kind:?}");
            };
            let witness = capability.reopen_witness();
            assert_eq!(witness.native_number(), 0);
            assert_eq!(witness.column_id(), 0);
            assert_eq!(witness.definition_page(), source.target_definition);
            assert_eq!(witness.descending(), descending);
            assert_eq!(witness.indexed_rows(), source.rows.len());
            assert_image(witness.expected_bytes(), &source.expected);
            assert_image(&fs::read(&path).unwrap(), &source.before);
            let journal = crate::journal_path(&path);
            assert!(!journal.exists());

            let saves = SaveWorker::new(|| {});
            let id = saves
                .start_index_creation(capability, journal.clone())
                .unwrap();
            let report = terminal(&saves);
            assert_eq!(report.id, id);
            assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
                if receipt.schema_change == Some(SaveSchemaChange::IndexCreated)
                && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
            assert!(saves.acknowledge(id));
            assert_image(&fs::read(&path).unwrap(), &source.expected);
            let Response::Opened {
                document_revision: fresh,
                ..
            } = request(&worker, Command::OpenIndexCreated { witness }).unwrap()
            else {
                panic!("expected guarded numeric index reopen");
            };
            assert!(fresh > revision);
            assert!(matches!(
                review(&worker, &inspected),
                Err(Error::StaleQuery)
            ));
            assert!(!journal.exists());
            assert_image(&fs::read(path).unwrap(), &source.expected);
        }
    }
}

#[test]
fn numeric_duplicate_index_and_foreign_inspection_refuse_without_publication() {
    let source = raw::source(Kind::Currency, Population::NullableDuplicate, false);
    let (_directory, path, worker, revision) = opened(&source.before);
    let inspected = inspect(
        &worker,
        revision,
        &format!(
            "CREATE INDEX [{}] ON [{}] ([{}])",
            source.index_name, source.table, source.column,
        ),
    );
    let (_foreign_directory, _foreign_path, foreign, _) = opened(&source.before);
    assert!(matches!(
        review(&foreign, &inspected),
        Err(Error::StaleQuery)
    ));
    assert_image(&fs::read(&path).unwrap(), &source.before);
    assert!(!crate::journal_path(&path).exists());

    let (_saved_directory, saved_path, saved, saved_revision) = opened(&source.expected);
    let duplicate = inspect(
        &saved,
        saved_revision,
        &format!(
            "CREATE INDEX [{}] ON [{}] ([{}])",
            source.index_name, source.table, source.column,
        ),
    );
    assert!(matches!(
        review(&saved, &duplicate),
        Err(Error::NumericIndexCreation(_))
    ));
    assert_image(&fs::read(&saved_path).unwrap(), &source.expected);
    assert!(!crate::journal_path(&saved_path).exists());
}
