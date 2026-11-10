//! Shared review/save/reopen owns complete native-width table creation.
use super::{opened_source, request, terminal};
use crate::{Command, Response, SaveOutcome, SaveSchemaChange, SaveWorker, journal_path};
use tablecraft_builders::table::{DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::fields, canonical_wide_table_creation as raw,
};

#[test]
fn wide_table_review_save_and_reopen_match_complete_independent_images() {
    let first = raw::source();
    let packed = raw::appended::source(&first.after, "WideAdded");
    let retained_before = fields::source(fields::Case::FirstNumber).bytes;
    let retained = raw::appended::source(&retained_before, "WideAdded");
    for (source, table, change) in [
        (
            first,
            "Table1",
            SaveSchemaChange::FirstTableCreated { column_count: 255 },
        ),
        (
            packed,
            "WideAdded",
            SaveSchemaChange::TableAdded { column_count: 255 },
        ),
        (
            retained,
            "WideAdded",
            SaveSchemaChange::TableAdded { column_count: 255 },
        ),
    ] {
        let (_directory, path, worker, document_revision) = opened_source(&source.before);
        let mut draft = TableDraft::initial();
        for index in 1..255 {
            draft
                .add_named_field(DraftFieldType::Number, &raw::field_name(index))
                .unwrap();
        }
        let Response::TableCreationReviewed { capability } = request(
            &worker,
            Command::ReviewTableCreation {
                document_revision,
                snapshot: draft.snapshot(table).unwrap(),
            },
        )
        .unwrap() else {
            panic!("expected complete native-width review");
        };
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_schema_change(), change);
        assert_image(witness.expected_bytes(), &source.after, table);
        assert_image(&std::fs::read(&path).unwrap(), &source.before, table);
        assert!(!journal_path(&path).exists());

        let saves = SaveWorker::new(|| {});
        let operation = saves
            .start_table_creation(capability, journal_path(&path))
            .unwrap();
        let report = terminal(&saves);
        assert!(
            matches!(report.outcome, SaveOutcome::Saved { .. }),
            "{table}: {report:?}"
        );
        assert!(saves.acknowledge(operation));
        assert_image(&std::fs::read(&path).unwrap(), &source.after, table);
        assert!(!journal_path(&path).exists());
        let Response::Opened {
            table_creation_available,
            table_design_available,
            ..
        } = request(&worker, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("expected exact wide-table reopen");
        };
        assert!(
            table_creation_available,
            "{table}: subsequent keyed creation"
        );
        assert!(
            table_design_available,
            "{table}: subsequent Design creation"
        );
        let Response::Data { columns, rows, .. } = request(
            &worker,
            Command::Page {
                table: table.into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected reopened wide table");
        };
        assert_eq!(columns.len(), 255);
        assert_eq!(columns.first().unwrap().name, "ID");
        assert_eq!(columns.last().unwrap().name, "Field254");
        assert!(rows.is_empty());
    }
}

fn assert_image(actual: &[u8], expected: &[u8], table: &str) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{table}: complete image length"
    );
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected),
        None,
        "{table}: first differing image byte",
    );
}
