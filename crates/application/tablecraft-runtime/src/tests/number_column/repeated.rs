//! The same public Number service consumes selected TCAC4 and existing witnesses.
use super::{
    Command, Response, SaveOutcome, SaveSchemaChange, SaveWorker, opened, raw, request, review,
    terminal,
};

#[test]
fn repeated_number_public_service_matches_all_three_independent_sdk_images() {
    for (case, table, column) in [
        (raw::Source::IdOnly, "Table1", "Score2"),
        (raw::Source::GuiRequiredZero, "Table1", "Extra2"),
        (raw::Source::DefaultOnly, "Items", "Extra2"),
    ] {
        let before = raw::repeated::source(case);
        let expected = raw::repeated::append(&before, column);
        let (directory, path, worker, revision) = opened(&before);
        let Response::NumberColumnAvailability { availability } = request(
            &worker,
            Command::InspectNumberColumnAvailability {
                document_revision: revision,
                table: table.into(),
            },
        )
        .unwrap() else {
            panic!("expected selected external-source availability");
        };
        assert!(availability.available);
        let (_, capability) = review(&worker, revision, table, column);
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), expected);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_number_column(capability, directory.path().join("repeat-number.journal"))
            .unwrap();
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected actual TCAC4 save: {report:?}");
        };
        assert_eq!(receipt.schema_change, Some(SaveSchemaChange::ColumnAdded));
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert!(receipt.query_change.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(saves.acknowledge(id));
        let Response::Opened {
            document_revision: fresh,
            ..
        } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
        else {
            panic!("expected full-image guarded repeated-column reopen");
        };
        assert_ne!(fresh, revision);
    }
}

#[test]
fn first_and_repeated_number_saves_compose_through_one_document_lifecycle() {
    let before = raw::source(raw::Source::IdOnly);
    let first = raw::append(&before, raw::Source::IdOnly, "Score");
    let expected = raw::repeated::append(&first, "Score2");
    let (directory, path, worker, mut revision) = opened(&before);
    let saves = SaveWorker::new(|| {});
    for (column, expected) in [("Score", &first), ("Score2", &expected)] {
        let (_, capability) = review(&worker, revision, "Table1", column);
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), expected);
        let id = saves
            .start_number_column(
                capability,
                directory.path().join(format!("{column}.journal")),
            )
            .unwrap();
        assert!(matches!(
            terminal(&saves).outcome,
            SaveOutcome::Saved { .. }
        ));
        assert!(saves.acknowledge(id));
        assert_eq!(std::fs::read(&path).unwrap(), *expected);
        let Response::Opened {
            document_revision, ..
        } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
        else {
            panic!("expected fresh exact Number document");
        };
        revision = document_revision;
    }
}

#[test]
fn unsupported_external_history_is_unavailable_without_retrying_first_add() {
    let mut before = raw::repeated::source(raw::Source::IdOnly);
    // Required false has a literal native0 carrier. A noncanonical1 is neither
    // absence nor the canonical true255; old-row history must not be normalized.
    before[11 * 4096 - 1] = 1;
    let (_directory, path, worker, revision) = opened(&before);
    let Response::NumberColumnAvailability { availability } = request(
        &worker,
        Command::InspectNumberColumnAvailability {
            document_revision: revision,
            table: "Table1".into(),
        },
    )
    .unwrap() else {
        panic!("expected refused external history observation");
    };
    assert!(!availability.available);
    assert!(
        request(
            &worker,
            Command::ReviewAddNumberColumn {
                document_revision: revision,
                table: "Table1".into(),
                column: "Score2".into(),
            }
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
