use super::*;
use crate::workspace::test_support::{finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_builders::table::DraftFieldType;
use tablecraft_testkit::fixtures::access::{canonical_blank_creation, mutation_fixture};

#[test]
fn ordinary_blank_open_does_not_invent_an_initial_table_draft() {
    let (_directory, path, original, harness) = opened(canonical_blank_creation::source());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(harness.state().state.active_object().is_none());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn deferred_open_preserves_the_exact_request_and_cancel_keeps_the_draft() {
    let (directory, path, original, mut harness) = opened(canonical_blank_creation::source());
    assert!(harness.state_mut().state.open_initial_table_draft());
    assert!(
        harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::ShortText)
    );
    let revision = harness.state().state.table_draft_revision();
    let generation = harness.state().generation;
    let next_path = directory.path().join("other.accdb");
    let next_bytes = mutation_fixture::with_rows(&[(1, 20)]);
    std::fs::write(&next_path, &next_bytes).unwrap();

    harness.state_mut().open_request(Command::Open {
        path: next_path.clone(),
        password: String::new(),
    });
    assert_eq!(harness.state().generation, generation);
    assert_eq!(harness.state().state.table_draft_revision(), revision);

    // A second transition cannot replace the operation awaiting confirmation.
    harness.state_mut().open_request(Command::Open {
        path: directory.path().join("must-not-open.accdb"),
        password: String::new(),
    });
    assert!(matches!(
        &harness.state().deferred_draft_action,
        Some(DeferredDraftAction::Open(command))
            if matches!(command.as_ref(), Command::Open { path, .. } if path == &next_path)
    ));
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(harness.state().generation, generation);
    assert_eq!(harness.state().state.table_draft_revision(), revision);
    assert!(harness.state().state.has_modified_table_drafts());

    harness.state_mut().open_request(Command::Open {
        path: next_path.clone(),
        password: String::new(),
    });
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Discard")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(harness.state().deferred_draft_action.is_none());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(harness.state().state.data.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read(&next_path).unwrap(), next_bytes);
}

#[test]
fn window_close_waits_for_explicit_discard_and_cancel_retains_the_draft() {
    let (_directory, path, original, mut harness) = opened(canonical_blank_creation::source());
    assert!(harness.state_mut().state.open_initial_table_draft());
    assert!(
        harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::LongInteger)
    );
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
            .contains(&ViewportCommand::CancelClose)
    );
    assert!(matches!(
        harness.state().deferred_draft_action,
        Some(DeferredDraftAction::CloseWindow)
    ));

    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().state.has_modified_table_drafts());
    assert!(harness.state().deferred_draft_action.is_none());

    harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Discard")
        .click_accesskit();
    harness.run_steps(1);
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(harness.state().deferred_draft_action.is_none());
    assert!(
        harness.output().viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&ViewportCommand::Close)
    );
    assert_eq!(std::fs::read(path).unwrap(), original);
}
