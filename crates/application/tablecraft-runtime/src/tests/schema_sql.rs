//! SQL schema review delegates to existing consuming capabilities and publication.
use super::{open, request, stored::opened, terminal};
use crate::{
    Command, Error, InspectedQuery, QueryOperation, Response, SaveOutcome, SaveSchemaChange,
    SaveWorker, Worker,
};
use std::{fs, sync::Arc};
use tablecraft_testkit::fixtures::access::canonical_table_drop as raw;

#[cfg(feature = "ace16-create")]
mod create;
#[cfg(feature = "ace16-create")]
mod field_drop;
#[cfg(feature = "ace16-create")]
mod index_creation;
#[cfg(feature = "ace16-create")]
mod make_table;

fn inspect(worker: &Worker, revision: u64, sql: &str) -> Arc<InspectedQuery> {
    let Response::QueryParameters { target, .. } = request(
        worker,
        Command::InspectQuery {
            document_revision: revision,
            sql: sql.into(),
        },
    )
    .unwrap() else {
        panic!("expected an issued syntax inspection");
    };
    assert_eq!(target.sql(), sql);
    assert_eq!(target.document_revision(), revision);
    target
}

fn review(worker: &Worker, target: &Arc<InspectedQuery>) -> Result<Response, Error> {
    request(
        worker,
        Command::ReviewSchemaSql {
            target: Arc::clone(target),
        },
    )
}

#[test]
fn issued_drop_table_reviews_then_saves_acknowledges_and_reopens_the_exact_native_image() {
    for (table, sql) in [
        ("Items", "DROP TABLE [iTeMs]"),
        ("雪🙂", "DROP TABLE [雪🙂]"),
    ] {
        let pair = raw::unindexed(5, table, false);
        let (_directory, path, worker, revision) = opened(&pair.before);
        let target = inspect(&worker, revision, sql);
        assert_eq!(target.operation(), QueryOperation::Schema);
        assert!(matches!(
            request(
                &worker,
                Command::RunQuery {
                    options: Default::default(),
                    target: Arc::clone(&target),
                    parameters: Vec::new(),
                }
            ),
            Err(Error::Query(tablecraft_query::QueryError::Unsupported))
        ));
        let Response::TableDropReviewed {
            review: facts,
            capability,
        } = review(&worker, &target).unwrap()
        else {
            panic!("SQL must return the existing consuming table DROP review");
        };
        assert_eq!(facts.table, table);
        assert_eq!(facts.document_revision, revision);
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), pair.after);
        assert_eq!(fs::read(&path).unwrap(), pair.before);
        let bundle = crate::journal_path(&path);
        assert!(!bundle.exists());
        let saves = SaveWorker::new(|| {});
        let id = saves.start_table_drop(capability, bundle.clone()).unwrap();
        let report = terminal(&saves);
        assert_eq!(report.id, id);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.schema_change == Some(SaveSchemaChange::TableDropped)
            && receipt.changed_rows.is_none() && receipt.query_change.is_none()));
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(fs::read(&path).unwrap(), pair.after);
        let Response::Opened {
            document_revision: fresh,
            tables,
            ..
        } = request(&worker, Command::OpenTableDropped { witness }).unwrap()
        else {
            panic!("expected guarded schema publication");
        };
        assert!(fresh > revision);
        assert!(tables.iter().all(|entry| entry.name != table));
        assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
        assert_eq!(fs::read(path).unwrap(), pair.after);
    }
}

#[test]
fn unsupported_schema_contexts_and_parameters_never_issue_a_save_capability() {
    let pair = raw::unindexed(5, "Items", false);
    let (_directory, path, worker, revision) = opened(&pair.before);
    for sql in [
        "CREATE TABLE Added (Id LONG DEFAULT 0)",
        "CREATE TEMPORARY TABLE Added (Id LONG)",
        "ALTER TABLE Items ADD COLUMN Note TEXT(20)",
        "ALTER TABLE Items ALTER COLUMN Note TEXT(20) DEFAULT Unknown",
        "ALTER TABLE Items ADD CONSTRAINT Rule CHECK (Id > 0)",
        "CREATE UNIQUE INDEX OtherIndex ON Items (Id)",
        "DROP VIEW Items",
        "SELECT 1",
    ] {
        let target = inspect(&worker, revision, sql);
        assert!(
            matches!(review(&worker, &target), Err(Error::SchemaSqlUnsupported)),
            "{sql}"
        );
        assert_eq!(fs::read(&path).unwrap(), pair.before);
        assert!(!crate::journal_path(&path).exists());
    }
    for sql in [
        "DROP TABLE Items; DELETE FROM Items",
        "PARAMETERS p LONG; DROP TABLE Items",
    ] {
        assert!(matches!(
            request(
                &worker,
                Command::InspectQuery {
                    document_revision: revision,
                    sql: sql.into(),
                }
            ),
            Err(Error::Parse(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), pair.before);
        assert!(!crate::journal_path(&path).exists());
    }
    assert_eq!(fs::read(path).unwrap(), pair.before);
}

#[test]
fn schema_review_requires_the_actual_issued_document_and_qualified_native_source() {
    let pair = raw::unindexed(5, "Items", false);
    let (_directory, path, worker, revision) = opened(&pair.before);
    let target = inspect(&worker, revision, "DROP TABLE Items");
    let (_other_directory, other_path, other_worker, _) = opened(&pair.before);
    assert!(matches!(
        review(&other_worker, &target),
        Err(Error::StaleQuery)
    ));
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
    assert_eq!(fs::read(path).unwrap(), pair.before);
    assert_eq!(fs::read(other_path).unwrap(), pair.before);

    let populated = raw::unindexed(5, "Items", true);
    let (_directory, path, worker, revision) = opened(&populated.before);
    let target = inspect(&worker, revision, "DROP TABLE Items");
    assert!(matches!(
        review(&worker, &target),
        Err(Error::TableDropIntent(_))
    ));
    assert_eq!(fs::read(&path).unwrap(), populated.before);
    assert!(!crate::journal_path(&path).exists());
}

#[test]
fn cancellation_is_checked_before_review_parsing_or_native_source_capture() {
    let pair = raw::unindexed(5, "Items", false);
    let (_directory, path, _worker, _) = opened(&pair.before);
    let (mut document, _) =
        crate::document::Document::open(&path, &mut String::new(), 7, &|| false).unwrap();
    let Response::QueryParameters { target, .. } = document
        .inspect_query(7, "DROP TABLE Items", &|| false)
        .unwrap()
    else {
        panic!("expected issued inspection");
    };
    assert!(matches!(
        document.review_schema_sql(&target, &crate::query::context::LocalClock, &|| true),
        Err(Error::Cancelled)
    ));
    assert_eq!(fs::read(&path).unwrap(), pair.before);
    assert!(!crate::journal_path(&path).exists());
}

#[test]
fn authenticated_read_only_input_cannot_issue_a_sql_schema_save_capability() {
    // Encryption behavior belongs to the engine's fixtures; this actual rewrite
    // supplies the same authenticated read-only source used by runtime actions.
    let sample = fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    let encrypted = tablecraft_ace::rewrite_encryption(
        &mut std::io::Cursor::new(sample),
        tablecraft_ace::EncryptionRewrite::Add {
            password: None,
            new_password: "test-only credential",
        },
        &mut crate::controls::budget(),
        &|| false,
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("read-only.accdb");
    fs::write(&path, encrypted.as_bytes()).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "test-only credential")).unwrap()
    else {
        panic!("expected authenticated native source");
    };
    let target = inspect(&worker, document_revision, "DROP TABLE Items");
    assert!(matches!(
        review(&worker, &target),
        Err(Error::Edit(crate::EditError::ReadOnlySource))
    ));
    assert_eq!(fs::read(&path).unwrap(), encrypted.as_bytes());
    assert!(!crate::journal_path(&path).exists());
}

#[cfg(not(feature = "ace16-create"))]
#[test]
fn journal_only_runtime_does_not_gain_native_column_schema_authority() {
    let pair = raw::unindexed(5, "Items", false);
    let (_directory, path, worker, revision) = opened(&pair.before);
    let target = inspect(&worker, revision, "ALTER TABLE Items DROP COLUMN Score");
    assert_eq!(target.operation(), QueryOperation::Schema);
    assert!(matches!(
        review(&worker, &target),
        Err(Error::SchemaSqlUnsupported)
    ));
    assert_eq!(fs::read(&path).unwrap(), pair.before);
    assert!(!crate::journal_path(&path).exists());
}

#[cfg(feature = "ace16-create")]
mod add_column;
