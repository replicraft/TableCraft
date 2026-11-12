//! Consuming SQL DROP preserves aliases, generators, records and fresh identity history.
use super::{inspect, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveSchemaChange, SaveWorker, Worker};
use std::{fs, path::Path};
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    self as raw, Population, dropping, history,
};

struct DropCase {
    source: raw::Source,
    before: Vec<u8>,
    after: Vec<u8>,
    logical: u32,
    physical: u32,
}

impl DropCase {
    fn exclusive(source: raw::Source, logical: u32, physical: u32) -> Self {
        let before = source.expected.to_vec();
        let after = dropping::after(&source);
        Self {
            source,
            before,
            after,
            logical,
            physical,
        }
    }

    fn alias() -> Self {
        let source = raw::protected_companion(false);
        let alias = dropping::surviving_alias(&source, "ScoreAlias");
        Self {
            source,
            before: alias.before,
            after: alias.expected,
            logical: 1,
            physical: 1,
        }
    }
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

fn save_drop(worker: &Worker, path: &Path, revision: u64, case: &DropCase) -> u64 {
    let sql = format!(
        "DROP INDEX [{}] ON [{}]",
        case.source.index_name, case.source.table
    );
    let target = inspect(worker, revision, &sql);
    let Response::IndexDropReviewed { capability } = review(worker, &target).unwrap() else {
        panic!("expected the actual native DROP original capability");
    };
    let witness = capability.reopen_witness();
    assert_eq!(witness.document_revision(), revision);
    assert_eq!(witness.table_name(), case.source.table);
    assert_eq!(witness.index_name(), case.source.index_name);
    assert_eq!(witness.definition_page(), case.source.target_definition);
    assert_eq!(witness.logical_index_number(), case.logical);
    assert_eq!(witness.physical_index_number(), case.physical);
    same(witness.expected_bytes(), &case.after);
    same(&fs::read(path).unwrap(), &case.before);
    let bundle = crate::journal_path(path);
    assert!(!bundle.exists());
    // Releasing a review is not confirmation or a native write.
    drop(capability);
    same(&fs::read(path).unwrap(), &case.before);
    let Response::IndexDropReviewed { capability } = review(worker, &target).unwrap() else {
        panic!("expected a fresh consuming DROP review");
    };
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let id = saves.start_index_drop(capability, bundle.clone()).unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, id);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::IndexDropped)
        && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
    assert!(saves.acknowledge(id));
    same(&fs::read(path).unwrap(), &case.after);
    let Response::Opened {
        document_revision: fresh,
        ..
    } = request(worker, Command::OpenIndexDropped { witness }).unwrap()
    else {
        panic!("expected actual DROP AFTER guarded publication");
    };
    assert!(fresh > revision);
    assert!(matches!(review(worker, &target), Err(Error::StaleQuery)));
    assert!(!bundle.exists());
    fresh
}

#[test]
fn drop_sql_retires_only_the_secondary_and_preserves_generated_primary_and_all_rows() {
    for descending in [false, true] {
        for case in [
            DropCase::exclusive(raw::source(Population::NullableDuplicate, descending), 0, 0),
            DropCase::exclusive(raw::protected_companion(descending), 1, 1),
        ] {
            let (_directory, path, worker, revision) = opened(&case.before);
            let Response::Data { rows: original, .. } = request(
                &worker,
                Command::Page {
                    table: case.source.table.into(),
                    offset: 0,
                },
            )
            .unwrap() else {
                panic!("expected original records");
            };
            save_drop(&worker, &path, revision, &case);
            let Response::Data { rows, .. } = request(
                &worker,
                Command::Page {
                    table: case.source.table.into(),
                    offset: 0,
                },
            )
            .unwrap() else {
                panic!("expected the same records after schema-only DROP");
            };
            assert_eq!(rows.len(), original.len());
            for (actual, old) in rows.iter().zip(&original) {
                assert_eq!(actual.len(), old.len());
                for (actual, old) in actual.iter().zip(old) {
                    assert_eq!(actual.text, old.text);
                    assert_eq!(actual.kind, old.kind);
                }
            }
            same(&fs::read(path).unwrap(), &case.after);
        }
    }
}

#[test]
fn alias_drop_then_sql_add_then_fresh_create_preserves_nondense_identity_history() {
    for descending in [false, true] {
        let case = DropCase::alias();
        let expected = history::source_with_direction(descending);
        let (_directory, path, worker, revision) = opened(&case.before);
        let revision = save_drop(&worker, &path, revision, &case);
        let target = inspect(
            &worker,
            revision,
            &format!(
                "ALTER TABLE [{}] ADD COLUMN OtherScore LONG",
                case.source.table
            ),
        );
        let Response::FieldAdditionReviewed { capability, .. } = review(&worker, &target).unwrap()
        else {
            panic!("expected real unadorned LONG ADD");
        };
        let witness = capability.reopen_witness();
        same(witness.expected_bytes(), &expected.before);
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_field_addition(capability, crate::journal_path(&path))
            .unwrap();
        assert!(
            matches!(terminal(&saves).outcome, SaveOutcome::Saved { receipt }
            if receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
            && receipt.matched_rows.is_none() && receipt.changed_rows.is_none())
        );
        assert!(saves.acknowledge(id));
        let Response::Opened {
            document_revision: revision,
            ..
        } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
        else {
            panic!("expected actual added-column publication");
        };
        same(&fs::read(&path).unwrap(), &expected.before);

        let sql = format!(
            "CREATE INDEX OtherLookup ON [{}] (OtherScore {})",
            case.source.table,
            if descending { "DESC" } else { "ASC" }
        );
        let target = inspect(&worker, revision, &sql);
        let Response::IndexCreationReviewed { capability } = review(&worker, &target).unwrap()
        else {
            panic!("count2 is occupied by logical2: this source requires fresh TCIC2");
        };
        let witness = capability.reopen_witness();
        assert_eq!(witness.native_number(), 4);
        assert_eq!(witness.indexed_rows(), case.source.rows.len());
        same(witness.expected_bytes(), &expected.expected);
        let id = saves
            .start_index_creation(capability, crate::journal_path(&path))
            .unwrap();
        assert!(
            matches!(terminal(&saves).outcome, SaveOutcome::Saved { receipt }
            if receipt.schema_change == Some(SaveSchemaChange::IndexCreated)
            && receipt.matched_rows.is_none() && receipt.changed_rows.is_none())
        );
        assert!(saves.acknowledge(id));
        assert!(matches!(
            request(&worker, Command::OpenIndexCreated { witness }),
            Ok(Response::Opened { .. })
        ));
        same(&fs::read(&path).unwrap(), &expected.expected);
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn drop_inspection_from_another_document_or_replaced_revision_cannot_grant_a_review() {
    let case = DropCase::alias();
    let (_directory, path, worker, revision) = opened(&case.before);
    let sql = format!(
        "DROP INDEX [{}] ON [{}]",
        case.source.index_name, case.source.table
    );
    let target = inspect(&worker, revision, &sql);
    let (_other_directory, _other_path, other, _) = opened(&case.before);
    assert!(matches!(review(&other, &target), Err(Error::StaleQuery)));
    request(&worker, crate::tests::open(&path, "")).unwrap();
    assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
    same(&fs::read(&path).unwrap(), &case.before);
    assert!(!crate::journal_path(&path).exists());
}
