//! Replace uses actual Find matches and the existing consuming offline save flow.
#![cfg(any(target_os = "macos", target_os = "linux"))]
use crate::workspace::Desktop;
use crate::workspace::test_support::{finish_reads, finish_save, focus_cell, opened, request_find};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::{Response, SaveOutcome};
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn replacement(harness: &mut Harness<'_, Desktop>, text: &str) {
    focus_cell(harness, "Score", "2", "20");
    request_find(harness, "20");
    finish_reads(harness);
    harness
        .get_by_role_and_label(Role::Tab, "Replace")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Replace With:")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, "Replace With:")
        .type_text(text);
    harness.run_steps(2);
    assert!(
        harness
            .state()
            .state
            .current_replace_find_request()
            .is_some()
    );
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn replace_current_integer_requires_review_and_apply_before_the_real_file_changes() {
    let (_directory, path, original, mut harness) = opened(
        mutation_fixture::with_rows_and_sort_order(&[(1, 10), (2, 20), (3, 30)], [9, 4, 0, 0]),
    );
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    replacement(&mut harness, "25");
    assert!(!harness.state().state.edit_dialog_open());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    harness
        .get_all_by_role_and_label(Role::Button, "Replace")
        .next_back()
        .expect("the dialog Replace action")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().state.find_window_open());
    assert!(harness.state().state.edit_dialog_open());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
            .value()
            .as_deref(),
        Some("25")
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Apply to original file")
            .is_none()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    harness
        .get_by_role_and_label(Role::Button, "Apply to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    let SaveOutcome::Saved { receipt, .. } = report.outcome else {
        panic!("the reviewed replacement must commit through SaveWorker");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_ne!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        revision
    );
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows[data.row_index(1).unwrap()][1].text, "25");
    harness
        .get_by_role_and_label(Role::Button, "First")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.offset, 0);
    assert_eq!(data.rows[1][1].text, "25");
    assert_eq!(data.rows[0][1].text, "10");
    assert_eq!(data.rows[2][1].text, "30");
    assert_ne!(std::fs::read(path).unwrap(), original);
}

#[test]
fn cancelled_completed_replace_receipt_cannot_reopen_edit_after_a_new_find_session() {
    let (_directory, path, original, mut harness) = opened(
        mutation_fixture::with_rows_and_sort_order(&[(1, 10), (2, 20), (3, 30)], [9, 4, 0, 0]),
    );
    replacement(&mut harness, "25");
    let request = harness
        .state()
        .state
        .current_replace_find_request()
        .unwrap();
    harness.state_mut().replace_find_match(request);
    let request_id = harness.state().generation;
    assert!(harness.state().state.busy);

    // Capture a real completed native receipt before UI publication, so closing
    // the dialog deterministically wins the response-delivery race.
    let deadline = Instant::now() + Duration::from_secs(10);
    let (generation, result) = loop {
        assert!(
            Instant::now() < deadline,
            "native replacement did not finish"
        );
        if let Some(reply) = harness.state().worker.poll() {
            break reply;
        }
        std::thread::yield_now();
    };
    assert_eq!(generation, request_id);
    let Response::EditDraft(draft) = result.unwrap() else {
        panic!("replacement must return an actual admitted edit draft");
    };
    let cancelled = harness.state_mut().state.close_find().unwrap();
    assert_eq!(cancelled, request_id);
    harness.state_mut().cancel_find(cancelled);
    assert!(!harness.state().state.busy);
    harness.state_mut().state.open_find();
    harness.run_steps(2);
    let generation = harness.state().generation;
    harness
        .state_mut()
        .publish_replace_find_draft(request_id, draft);
    harness.run_steps(2);
    assert_eq!(harness.state().generation, generation);
    assert!(harness.state().state.find_window_open());
    assert!(!harness.state().state.edit_dialog_open());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows[data.row_index(1).unwrap()][1].text, "20");
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn accepted_replace_draft_can_be_cancelled_before_review_without_saving() {
    let (_directory, path, original, mut harness) = opened(
        mutation_fixture::with_rows_and_sort_order(&[(1, 10), (2, 20), (3, 30)], [9, 4, 0, 0]),
    );
    replacement(&mut harness, "25");
    harness
        .get_all_by_role_and_label(Role::Button, "Replace")
        .next_back()
        .expect("the dialog Replace action")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(harness.state().state.edit_dialog_open());
    assert!(!harness.state().state.find_window_open());
    assert!(harness.state().editing.save_report().is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Apply to original file")
            .is_none()
    );

    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.edit_dialog_open());
    assert!(!harness.state().state.busy);
    assert!(harness.state().editing.save_report().is_none());
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Review change")
            .is_none()
    );
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows[data.row_index(1).unwrap()][1].text, "20");
    assert_eq!(std::fs::read(path).unwrap(), original);
}
