//! The actual GUI draft declaration/save producer must match independent sources.
use super::{opened, request, terminal};
use crate::{Command, Response, SaveOutcome, SaveWorker, journal_path};
use tablecraft_builders::table::{DraftDefaultMode, DraftFieldType, DraftNumberSize, TableDraft};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;

#[test]
fn actual_number_draft_save_matches_independent_gui_policy_source_before_add_column() {
    for case in [raw::Source::IdOnly, raw::Source::GuiRequiredZero] {
        let (_directory, path, worker, document_revision) = opened();
        let mut draft = TableDraft::initial();
        if matches!(case, raw::Source::GuiRequiredZero) {
            draft
                .add_named_field(DraftFieldType::Number, "Score")
                .unwrap();
            let field = &draft.fields()[1];
            assert_eq!(field.number_size(), Some(DraftNumberSize::LongInteger));
            assert_eq!(field.required(), Some(false));
            assert_eq!(field.default_mode(), &DraftDefaultMode::TypeDefault);
        }
        let Response::TableCreationReviewed { capability } = request(
            &worker,
            Command::ReviewTableCreation {
                document_revision,
                snapshot: draft.snapshot("Table1").unwrap(),
            },
        )
        .unwrap() else {
            panic!("expected exact GUI table draft source producer");
        };
        let expected = raw::source(case);
        let witness = capability.reopen_witness();
        assert_eq!(witness.expected_bytes(), expected);
        let saves = SaveWorker::new(|| {});
        let operation = saves
            .start_table_creation(capability, journal_path(&path))
            .unwrap();
        let report = terminal(&saves);
        assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
        assert!(saves.acknowledge(operation));
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(matches!(
            request(&worker, Command::OpenTableCreated { witness }).unwrap(),
            Response::Opened { .. }
        ));
    }
}
