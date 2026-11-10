//! Source capacity is derived from native storage, separately from draft facts.
use super::{opened_source, request, terminal};
use crate::{Command, Response, SaveOutcome, SaveWorker, journal_path};
use tablecraft_builders::table::{DraftFieldType, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_mixed_add_table;

#[test]
fn keyed_and_keyless_designs_save_across_the_native_map_window() {
    let source = canonical_mixed_add_table::capacity_source();
    for keyed in [false, true] {
        let (_directory, path, reader, revision) = opened_source(&source.bytes);
        let Response::Opened {
            table_creation_available,
            table_design_available,
            document_revision,
            ..
        } = request(
            &reader,
            Command::Open {
                path: path.clone(),
                password: String::new(),
            },
        )
        .unwrap()
        else {
            panic!("expected a readable native append source");
        };
        assert_ne!(document_revision, revision);
        assert!(table_creation_available);
        assert!(table_design_available);

        let mut draft = TableDraft::design();
        draft
            .add_named_field(DraftFieldType::ShortText, "Name")
            .unwrap();
        let key = draft
            .add_named_field(DraftFieldType::Number, "ActualKey")
            .unwrap();
        if keyed {
            draft.set_primary_key(Some(key)).unwrap();
        }
        let snapshot = draft.snapshot("Last").unwrap();
        let Response::TableCreationReviewed { capability } = request(
            &reader,
            Command::ReviewTableCreation {
                document_revision,
                snapshot,
            },
        )
        .unwrap() else {
            panic!("expected exact manual declaration review, keyed={keyed}");
        };
        assert!(
            std::fs::read(&path).unwrap() == source.bytes,
            "review changed the source"
        );
        assert!(!journal_path(&path).exists());
        let witness = capability.reopen_witness();
        let saver = SaveWorker::new(|| {});
        let operation = saver
            .start_table_creation(capability, journal_path(&path))
            .unwrap();
        let outcome = terminal(&saver).outcome;
        assert!(
            matches!(outcome, SaveOutcome::Saved { .. }),
            "keyed={keyed}: {outcome:?}"
        );
        assert!(saver.acknowledge(operation));
        assert!(
            std::fs::read(&path).unwrap() == witness.expected_bytes(),
            "saved image differs from reviewed witness"
        );
        let Response::Opened {
            table_creation_available,
            table_design_available,
            ..
        } = request(&reader, Command::OpenTableCreated { witness }).unwrap()
        else {
            panic!("expected exact saved manual schema reopening");
        };
        assert!(table_creation_available);
        assert!(table_design_available);
        let Response::Data { columns, rows, .. } = request(
            &reader,
            Command::Page {
                table: "Last".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected saved empty manual table");
        };
        assert_eq!(
            columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>(),
            ["Name", "ActualKey"]
        );
        assert!(rows.is_empty());
        assert!(!journal_path(&path).exists());
    }
}
