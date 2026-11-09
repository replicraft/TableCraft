//! Real host dispatch and native save ownership through headless insertion widgets.
use super::*;
use crate::workspace::test_support::{self, finish_reads, finish_save};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::path::PathBuf;
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

mod number_column;
mod retry;

fn opened() -> (
    tempfile::TempDir,
    PathBuf,
    Vec<u8>,
    Harness<'static, Desktop>,
) {
    let original = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    let result = test_support::opened(original);
    assert_eq!(result.3.state().state.data.as_ref().unwrap().rows.len(), 1);
    result
}

fn enter_id(harness: &mut Harness<'_, Desktop>, text: &str) {
    let role = input_role(harness);
    harness.get_by_role_and_label(role, "Id").focus();
    harness.run_steps(1);
    let role = input_role(harness);
    harness.get_by_role_and_label(role, "Id").type_text(text);
    harness.run_steps(2);
    let role = input_role(harness);
    assert_eq!(
        harness.get_by_role_and_label(role, "Id").value().as_deref(),
        Some(text),
    );
}

fn input_role(harness: &Harness<'_, Desktop>) -> Role {
    if harness
        .query_by_role_and_label(Role::TextInput, "Id")
        .is_some()
    {
        Role::TextInput
    } else {
        Role::MultilineTextInput
    }
}

fn begin(harness: &mut Harness<'_, Desktop>, id: &str) {
    // The native Ribbon caption starts the same source-bound appended row as
    // the datasheet and record navigator affordances.
    harness
        .get_by_role_and_label(Role::Button, "New")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Id")
            .is_some()
    );
    assert!(harness.query_by_role(Role::MultilineTextInput).is_none());
    enter_id(harness, id);
}

fn review(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

#[test]
fn reviewed_insert_keeps_its_terminal_owner_until_ack_then_refreshes_the_same_table() {
    let (_directory, path, original, mut harness) = opened();
    begin(&mut harness, "4");
    review(&mut harness);
    let review_id = harness.state().editing.insert_review.as_ref().unwrap().0;
    assert!(harness.state().state.can_apply_insert(review_id));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());

    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    assert!(harness.state().editing.has_save());
    let report = finish_save(&mut harness);
    let save = harness.state().editing.save.as_ref().unwrap();
    assert!(matches!(save.owner, SaveOwner::Insert));
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1))
    );
    assert_eq!(harness.state().editing.worker.poll(), Some(report));
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 1);

    harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    harness.run_steps(1);
    assert!(
        harness.output().viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(harness.state().editing.has_save());
    assert_eq!(harness.state().editing.worker.poll(), Some(report));

    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().editing.refresh.is_none());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Items");
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.rows[1][0].text, "4");
    assert!(matches!(
        data.rows[1][1].kind,
        tablecraft_ui::CellKind::Scalar { is_null: true, .. }
    ));
    assert!(!journal_path(&path).exists());
}

#[test]
fn invalid_input_and_changed_or_cancelled_drafts_never_apply_an_old_review() {
    let (_directory, path, original, mut harness) = opened();
    begin(&mut harness, "bad");
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);

    let role = input_role(&harness);
    harness.get_by_role_and_label(role, "Id").focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    enter_id(&mut harness, "4");
    review(&mut harness);
    let review_id = harness.state().editing.insert_review.as_ref().unwrap().0;
    assert!(harness.state().state.can_apply_insert(review_id));

    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Id")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    enter_id(&mut harness, "5");
    assert!(!harness.state().state.can_apply_insert(review_id));
    harness.state_mut().apply_insert(review_id);
    assert!(!harness.state().editing.has_save());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.insertion_dialog_open());
    assert!(harness.state().editing.insert_review.is_none());
    harness.state_mut().apply_insert(review_id);
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
}

#[test]
fn pending_record_survives_host_open_create_and_window_close_requests() {
    let (directory, path, original, mut harness) = opened();
    begin(&mut harness, "4");
    let generation = harness.state().generation;
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    let other = directory.path().join("other.accdb");
    std::fs::write(&other, &original).unwrap();

    harness.state_mut().open_request(Command::Open {
        path: other,
        password: String::new(),
    });
    let destination = directory.path().join("new.accdb");
    harness
        .state_mut()
        .create_database(tablecraft_ui::CreationRequest {
            id: 1,
            destination: destination.to_str().unwrap().to_owned(),
        });
    assert_eq!(harness.state().generation, generation);
    assert_eq!(
        harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision,
        revision
    );
    assert!(harness.state().state.insertion_dialog_open());
    assert!(!destination.exists());

    harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    harness.run_steps(1);
    assert!(
        harness.output().viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(harness.state().state.insertion_dialog_open());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
}
