//! Real SQL review consumes the native original index plan and exact read witness.
use super::{inspect, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{
    CellKind, Command, Error, QueryOperation, Response, SaveOutcome, SaveSchemaChange, SaveWorker,
};
use std::fs;
use tablecraft_ace::{
    EditError, FreshIndexCreateIntentError, IndexConstraintViolation, InspectError,
};
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, Population};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn create_index_reviews_native_field_zero_saves_schema_only_and_reopens_unchanged_rows() {
    for population in [Population::Empty, Population::NullableDuplicate] {
        for descending in [false, true] {
            let source = raw::source(population, descending);
            let (_directory, path, worker, revision) = opened(&source.before);
            let sql = format!(
                "CREATE INDEX [{}] ON [{}] ([{}] {})",
                source.index_name,
                source.table,
                source.column,
                if descending { "DESC" } else { "ASC" }
            );
            let target = inspect(&worker, revision, &sql);
            assert_eq!(target.operation(), QueryOperation::Schema);
            let Response::IndexCreationReviewed { capability } = review(&worker, &target).unwrap()
            else {
                panic!("expected the original native index capability");
            };
            let witness = capability.reopen_witness();
            assert_eq!(witness.table_name(), source.table);
            assert_eq!(witness.index_name(), source.index_name);
            assert_eq!(witness.column_name(), source.column);
            assert_eq!(
                witness.native_number(),
                0,
                "ordinary field zero is not a generator"
            );
            assert_eq!(witness.definition_page(), source.target_definition);
            assert_eq!(witness.column_id(), 0);
            assert_eq!(witness.descending(), descending);
            assert_eq!(witness.indexed_rows(), source.rows.len());
            assert_image(witness.expected_bytes(), &source.expected);
            assert_image(&fs::read(&path).unwrap(), &source.before);
            let bundle = crate::journal_path(&path);
            assert!(!bundle.exists());

            let saves = SaveWorker::new(|| {});
            let id = saves
                .start_index_creation(capability, bundle.clone())
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
                tables,
                ..
            } = request(&worker, Command::OpenIndexCreated { witness }).unwrap()
            else {
                panic!("expected guarded index publication");
            };
            assert!(fresh > revision);
            assert!(tables.iter().any(|table| table.name == source.table));
            assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
            let Response::Data { rows, .. } = request(
                &worker,
                Command::Page {
                    table: source.table.into(),
                    offset: 0,
                },
            )
            .unwrap() else {
                panic!("expected original source records");
            };
            assert_eq!(rows.len(), source.rows.len());
            for (actual, original) in rows.iter().zip(&source.rows) {
                assert!(
                    matches!(actual[0].kind, CellKind::Scalar { is_null, .. } if is_null == original.score.is_none())
                );
                if let Some(value) = original.score {
                    assert_eq!(actual[0].text, value.to_string());
                }
            }
            assert!(!bundle.exists());
            assert_image(&fs::read(&path).unwrap(), &source.expected);
        }
    }
}

#[test]
fn create_index_rejects_unimplemented_flags_fields_duplicates_and_foreign_inspection() {
    let source = raw::source(Population::NullableDuplicate, false);
    let (_directory, path, worker, revision) = opened(&source.before);
    let duplicate_unique = inspect(
        &worker,
        revision,
        "CREATE UNIQUE INDEX Ix ON Archive (CopiedScore)",
    );
    assert!(matches!(
        review(&worker, &duplicate_unique),
        Err(Error::FreshIndexCreation(
            FreshIndexCreateIntentError::Edit(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            )))
        ))
    ));
    assert_image(&fs::read(&path).unwrap(), &source.before);
    assert!(!crate::journal_path(&path).exists());

    for sql in [
        "CREATE INDEX Ix ON Archive (CopiedScore) WITH PRIMARY",
        "CREATE INDEX Ix ON Archive (CopiedScore) WITH IGNORE NULL",
        "CREATE INDEX Ix ON Archive (CopiedScore) WITH DISALLOW NULL",
        "CREATE INDEX Ix ON Archive (CopiedScore, CopiedScore)",
    ] {
        let target = inspect(&worker, revision, sql);
        assert!(matches!(
            review(&worker, &target),
            Err(Error::SchemaSqlUnsupported)
        ));
        assert_image(&fs::read(&path).unwrap(), &source.before);
    }
    let target = inspect(
        &worker,
        revision,
        "CREATE INDEX Ix ON Archive (CopiedScore)",
    );
    let (_other_directory, _other_path, other_worker, _) = opened(&source.before);
    assert!(matches!(
        review(&other_worker, &target),
        Err(Error::StaleQuery)
    ));
    assert!(!crate::journal_path(&path).exists());

    let (_indexed_directory, indexed_path, indexed_worker, indexed_revision) =
        opened(&source.expected);
    let duplicate = inspect(
        &indexed_worker,
        indexed_revision,
        &format!(
            "CREATE INDEX [{}] ON [{}] ([{}])",
            source.index_name, source.table, source.column,
        ),
    );
    assert!(matches!(
        review(&indexed_worker, &duplicate),
        Err(Error::IndexCreation(_))
    ));
    assert_image(&fs::read(indexed_path).unwrap(), &source.expected);

    let companion = raw::protected_companion(false);
    let (_directory, path, worker, revision) = opened(&companion.before);
    let sql = format!("CREATE INDEX Ix ON [{}] (Label)", companion.table);
    let target = inspect(&worker, revision, &sql);
    assert!(matches!(
        review(&worker, &target),
        Err(Error::Native(tablecraft_ace::InspectError::Unsupported))
    ));
    assert_image(&fs::read(path).unwrap(), &companion.before);
}

mod numeric;
