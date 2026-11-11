//! Native identity survives SQL spelling, non-tail retirement and retained holes.
use super::{inspect, review};
use crate::tests::{request, stored::opened, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveSchemaChange, SaveWorker};
use std::fs;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, fields, typed_fields,
};

#[test]
fn sql_drop_column_reviews_native_tail_and_non_tail_identities_then_saves_exact_images() {
    let source = fields::source(fields::Case::FirstNumber);
    let before = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    for (written, actual, native_number) in [("sCoRe", "Score", 1), ("aMoUnT", "Amount", 2)] {
        let after =
            field_changes::drop_field(&before, source.definition, source.catalog_root, actual);
        let (_directory, path, worker, revision) = opened(&before);
        let target = inspect(
            &worker,
            revision,
            &format!("ALTER TABLE [tAbLe1] DROP COLUMN [{written}]"),
        );
        let Response::FieldDropReviewed {
            review: facts,
            capability,
        } = review(&worker, &target).unwrap()
        else {
            panic!("SQL must return the existing consuming field DROP review");
        };
        assert_eq!(
            (
                facts.table.as_str(),
                facts.column.as_str(),
                facts.native_number
            ),
            ("Table1", actual, native_number)
        );
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), after);
        assert_eq!(fs::read(&path).unwrap(), before);
        let bundle = crate::journal_path(&path);
        assert!(!bundle.exists());
        let saves = SaveWorker::new(|| {});
        let id = saves.start_field_drop(capability, bundle.clone()).unwrap();
        let report = terminal(&saves);
        assert_eq!(report.id, id);
        assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
            if receipt.schema_change == Some(SaveSchemaChange::ColumnDropped)
            && receipt.changed_rows.is_none() && receipt.query_change.is_none()));
        assert!(saves.acknowledge(id));
        assert!(!bundle.exists());
        assert_eq!(fs::read(&path).unwrap(), after);
        let Response::Opened {
            document_revision: fresh,
            ..
        } = request(&worker, Command::OpenFieldDropped { witness }).unwrap()
        else {
            panic!("expected guarded schema publication");
        };
        assert!(fresh > revision);
        assert!(matches!(review(&worker, &target), Err(Error::StaleQuery)));
        let Response::Data { columns, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected the surviving native columns");
        };
        assert!(columns.iter().all(|column| column.name != actual));
        assert_eq!(fs::read(path).unwrap(), after);
    }
}

#[test]
fn primary_index_and_unknown_field_requests_keep_the_native_refusal_and_original_bytes() {
    let source = fields::source(fields::Case::FirstNumber);
    let (_directory, path, worker, revision) = opened(&source.bytes);
    for column in ["ID", "Missing"] {
        let target = inspect(
            &worker,
            revision,
            &format!("ALTER TABLE Table1 DROP COLUMN [{column}]"),
        );
        assert!(matches!(
            review(&worker, &target),
            Err(Error::FieldDropSource(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), source.bytes);
        assert!(!crate::journal_path(&path).exists());
    }
}
