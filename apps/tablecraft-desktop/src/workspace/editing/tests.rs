//! Real host admission and acknowledgment over the existing runtime/testkit path.
use super::*;
use eframe::App;
use std::time::{Duration, Instant};
use tablecraft_runtime::Response;
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn finish_reads(desktop: &mut Desktop, context: &egui::Context) {
    let mut frame = eframe::Frame::_new_kittest();
    let deadline = Instant::now() + Duration::from_secs(10);
    while desktop.state.busy {
        assert!(Instant::now() < deadline, "read did not finish");
        desktop.logic(context, &mut frame);
        std::thread::yield_now();
    }
    assert!(desktop.state.error.is_none());
}

fn opened() -> (tempfile::TempDir, PathBuf, Vec<u8>, egui::Context, Desktop) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("editable.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let context = egui::Context::default();
    let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
    finish_reads(&mut desktop, &context);
    desktop.submit(Command::Page {
        table: "Items".into(),
        offset: 0,
    });
    finish_reads(&mut desktop, &context);
    (directory, path, original, context, desktop)
}

fn begin(desktop: &mut Desktop) -> (u64, u64, EditInput) {
    let target = EditTarget {
        document_revision: desktop.state.database.as_ref().unwrap().document_revision,
        table: "Items".into(),
        column: "Score".into(),
        parent_ordinal: 1,
    };
    desktop.begin_edit(target);
    let request_id = desktop.generation;
    let deadline = Instant::now() + Duration::from_secs(10);
    let response = loop {
        if let Some((generation, response)) = desktop.worker.poll() {
            assert_eq!(generation, request_id);
            break response.unwrap();
        }
        assert!(Instant::now() < deadline, "draft did not finish");
        std::thread::yield_now();
    };
    let Response::EditDraft(draft) = response else {
        panic!("expected actual native draft");
    };
    let draft_id = draft.id;
    let input = EditInput {
        text: draft.initial.text.clone(),
        is_null: draft.initial.is_null,
    };
    assert_eq!(input.text, "20");
    desktop.state.busy = false;
    desktop.publish_edit_draft(request_id, draft);
    (request_id, draft_id, input)
}

fn reviewed(desktop: &mut Desktop, context: &egui::Context) -> u64 {
    let (_, draft_id, input) = begin(desktop);
    desktop.review_edit(draft_id, input);
    finish_reads(desktop, context);
    let review_id = desktop.editing.review.as_ref().unwrap().0;
    assert!(desktop.state.can_apply_edit(review_id));
    review_id
}

fn finish_save(desktop: &mut Desktop) -> SaveReport {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        desktop.poll_saves();
        if let Some(report) = desktop.editing.save.as_ref().and_then(|save| save.report) {
            return report;
        }
        assert!(Instant::now() < deadline, "save did not finish");
        std::thread::yield_now();
    }
}

#[test]
fn stale_review_apply_and_close_do_not_replace_current_requests_or_touch_the_file() {
    let (_directory, path, original, context, mut desktop) = opened();
    let (draft_request, draft_id, input) = begin(&mut desktop);
    let before = desktop.generation;
    desktop.review_edit(
        draft_id,
        EditInput {
            text: "999".into(),
            is_null: false,
        },
    );
    assert_eq!(desktop.generation, before);
    assert!(!desktop.state.busy);
    desktop.review_edit(draft_id + 1, input.clone());
    assert_eq!(desktop.generation, before);

    desktop.review_edit(draft_id, input);
    let current = desktop.generation;
    assert_ne!(current, draft_request);
    desktop.close_edit(draft_request);
    assert_eq!(desktop.generation, current);
    assert!(desktop.state.busy);
    assert!(desktop.state.edit_dialog_open());
    finish_reads(&mut desktop, &context);
    let review_id = desktop.editing.review.as_ref().unwrap().0;
    desktop.apply_edit(review_id + 1);
    assert!(!desktop.editing.has_save());
    assert_eq!(desktop.editing.review.as_ref().unwrap().0, review_id);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());

    desktop.open();
    finish_reads(&mut desktop, &context);
    desktop.apply_edit(review_id);
    assert!(!desktop.editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
}

#[test]
fn acknowledged_noop_save_refreshes_the_original_table_and_blocks_conflicting_actions() {
    let (_directory, path, original, context, mut desktop) = opened();
    let review_id = reviewed(&mut desktop, &context);
    desktop.apply_edit(review_id);
    assert!(desktop.editing.has_save());
    let read_generation = desktop.generation;
    desktop.submit(Command::Query {
        sql: "SELECT 1".into(),
    });
    desktop.open();
    desktop.file_selected(Some(path.with_file_name("another.mdb")));
    assert_eq!(desktop.generation, read_generation);
    assert_eq!(desktop.state.path, path.to_string_lossy());

    let report = finish_save(&mut desktop);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected actual finalized no-op save: {report:?}");
    };
    assert_eq!(receipt.changed_rows, Some(0));
    assert_eq!(receipt.matched_rows, Some(1));
    desktop.acknowledge_save(report.id + 1);
    assert!(desktop.editing.has_save());
    desktop.acknowledge_save(report.id);
    assert!(!desktop.editing.has_save());
    assert!(desktop.state.busy);
    let refresh_generation = desktop.generation;
    assert_eq!(
        desktop.editing.refresh.as_ref().unwrap().0,
        refresh_generation
    );
    finish_reads(&mut desktop, &context);
    assert!(desktop.editing.refresh.is_none());
    let data = desktop
        .state
        .data
        .as_ref()
        .expect("fresh table after acknowledgment");
    assert_eq!(data.table, "Items");
    assert_eq!(data.offset, 0);
    assert_eq!(data.rows[1][1].text, "20");
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
}

#[test]
fn refresh_binding_does_not_follow_an_unrelated_open_generation() {
    let (_directory, _path, _original, _context, mut desktop) = opened();
    let current = desktop.generation;
    desktop.editing.refresh = Some((current, "Items".into(), 1));
    desktop.refresh_after_save(current + 1);
    assert_eq!(desktop.generation, current);
    assert!(desktop.editing.refresh.is_none());
    assert!(!desktop.state.busy);
}

#[test]
fn close_is_vetoed_until_the_real_terminal_save_is_acknowledged() {
    let (_directory, _path, _original, context, mut desktop) = opened();
    let review_id = reviewed(&mut desktop, &context);
    desktop.apply_edit(review_id);
    let report = finish_save(&mut desktop);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = context.run_ui(input, |ui| desktop.guard_save_close(ui.ctx()));
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    output.textures_delta.clear();
    assert!(desktop.editing.has_save());
    assert_eq!(desktop.editing.worker.poll(), Some(report));
    desktop.acknowledge_save(report.id);
    assert!(!desktop.editing.has_save());
}

#[test]
fn release_failure_preserves_completed_receipt_without_promoting_it_to_current_outcome() {
    let failure = SaveFailure {
        kind: tablecraft_runtime::SaveFailureKind::Release,
        phase: Some(tablecraft_runtime::SavePhase::Release),
        state: Some(tablecraft_runtime::SaveSessionState::ReleaseUncertain),
        current_outcome: None,
        last_completed: Some(SaveReceipt {
            completion: SaveCompletion::Applied,
            known: SaveKnownOutcome::Committed,
            matched_rows: Some(3),
            changed_rows: Some(2),
            schema_change: None,
        }),
        can_recover: false,
        can_release: false,
    };
    for (locale, expected) in [
        (
            tablecraft_ui::Locale::EnUs,
            "Save did not finish normally (lock release was not acknowledged). Current operation outcome: not established. Last completed operation: applied; outcome: committed. Matched rows: 3. Changed rows: 2. Do not retry automatically.",
        ),
        (
            tablecraft_ui::Locale::De,
            "Das Speichern wurde nicht ordnungsgemäß abgeschlossen (die Freigabe der Sperre wurde nicht bestätigt). Ergebnis des aktuellen Vorgangs: nicht festgestellt. Letzter abgeschlossener Vorgang: angewendet; Ergebnis: übernommen. Übereinstimmende Zeilen: 3. Geänderte Zeilen: 2. Nicht automatisch erneut versuchen.",
        ),
    ] {
        let localizer = Localizer::new(locale).unwrap();
        let outcome = save_outcome(&localizer, SaveOutcome::Failed(failure));
        let EditSaveOutcome::Retained {
            message,
            can_recover,
            can_release,
        } = outcome
        else {
            panic!("release failure must remain retained");
        };
        assert!(!can_recover);
        assert!(!can_release);
        // Fluent isolation controls surround typed substitutions without changing receipt text.
        assert_eq!(message.replace(['\u{2068}', '\u{2069}'], ""), expected);
        assert_eq!(localizer.diagnostics(), Default::default());
    }
}

#[test]
fn released_resources_keep_current_and_prior_receipt_facts_distinct() {
    for (completion, label, known) in [
        (
            SaveCompletion::Recovered,
            "recovered",
            SaveKnownOutcome::Committed,
        ),
        (
            SaveCompletion::Confirmed,
            "confirmed",
            SaveKnownOutcome::ReplacementVerified,
        ),
    ] {
        let outcome = save_outcome(
            &Localizer::default(),
            SaveOutcome::Released {
                current_outcome: Some(SaveKnownOutcome::RolledBack),
                last_completed: Some(SaveReceipt {
                    completion,
                    known,
                    matched_rows: None,
                    changed_rows: None,
                    schema_change: None,
                }),
            },
        );
        let EditSaveOutcome::Failed(message) = outcome else {
            panic!("release alone must not become a successful save");
        };
        let message = message.replace(['\u{2068}', '\u{2069}'], "");
        assert!(message.contains("Recovery files may remain"));
        assert!(message.contains("Current operation outcome: rolled back."));
        assert!(message.contains(&format!("Last completed operation: {label};")));
        assert!(message.contains(&known_result(&Localizer::default(), Some(known))));
        assert!(!message.contains("Matched rows:"));
        assert!(!message.contains("Changed rows:"));
    }
}

#[test]
fn absent_receipt_does_not_invent_completed_operations_or_counts() {
    let outcome = save_outcome(
        &Localizer::default(),
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None,
        },
    );
    let EditSaveOutcome::Failed(message) = outcome else {
        panic!("released unknown result is not a save receipt");
    };
    let message = message.replace(['\u{2068}', '\u{2069}'], "");
    assert!(message.contains("Current operation outcome: not established."));
    assert!(message.contains("No completed-operation receipt is available."));
    assert!(!message.contains("Last completed operation:"));
    assert!(!message.contains("Matched rows:"));
    assert!(!message.contains("Changed rows:"));
}
