//! Real alias authoring preserves incomplete data across host decisions and saves.
use super::{design, opened, run, scratch, sql};
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use crate::workspace::test_support::cell_edit;
use crate::workspace::{Desktop, table_drafts::DeferredDraftAction, test_support::finish_reads};
use egui::{Key, ViewportCommand, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_runtime::Command;
use tablecraft_testkit::fixtures::access::mutation_fixture;
use tablecraft_ui::{ObjectKind, ObjectTarget};

fn field(harness: &mut Harness<'_, Desktop>, index: usize, text: &str) {
    harness
        .get_all_by_role_and_label(Role::TextInput, "Field expression")
        .nth(index)
        .unwrap()
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text(text.into()));
    harness.run_steps(2);
}

fn show_items(harness: &mut Harness<'_, Desktop>) {
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness.state_mut().open_object(ObjectTarget {
        document_revision: revision,
        kind: ObjectKind::Table {
            name: "Items".into(),
        },
    });
    harness.run_steps(2);
    finish_reads(harness);
}

fn close_event(harness: &mut Harness<'_, Desktop>) {
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
}

#[test]
fn actual_field_alias_edit_build_and_run_keep_heading_and_native_rows() {
    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    scratch(&mut harness);
    sql(&mut harness, "SELECT Id,Score FROM Items ORDER BY Id");
    run(&mut harness);
    design(&mut harness);
    field(&mut harness, 1, "[Score: view]: Score");
    harness
        .get_all_by_role_and_label(Role::TextInput, "Field expression")
        .next()
        .unwrap()
        .focus();
    harness.run_steps(2);
    assert!(!harness.state().state.has_unresolved_query_design_edits());
    assert_eq!(
        harness.state().state.query_design_draft().unwrap().columns[1]
            .alias
            .as_deref(),
        Some("Score: view")
    );
    run(&mut harness);
    let data = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(data.columns[1].name, "Score: view");
    assert_eq!(
        data.rows
            .iter()
            .map(|row| row[1].text.as_str())
            .collect::<Vec<_>>(),
        ["10", "20", "30"]
    );
    assert!(data.source_revision.is_none());
    assert!(data.row_ordinals.is_none());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn parked_incomplete_alias_blocks_destructive_open_queue_and_close_until_explicit_cancel() {
    let (directory, path, original, mut harness) = opened(mutation_fixture::database());
    scratch(&mut harness);
    sql(&mut harness, "SELECT Id,Score FROM Items");
    run(&mut harness);
    design(&mut harness);
    let query_tab = harness.state().state.active_object().unwrap();

    field(&mut harness, 1, "Chosen: IIf(");
    harness
        .get_all_by_role_and_label(Role::TextInput, "Field expression")
        .next()
        .unwrap()
        .focus();
    harness.run_steps(2);
    assert!(harness.state().state.has_unresolved_query_design_edits());
    let generation = harness.state().generation;
    let run = harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap();
    assert!(run.accesskit_node().is_disabled());
    assert_eq!(harness.state().generation, generation);

    show_items(&mut harness);
    assert_ne!(
        harness.state().state.active_object().unwrap().id,
        query_tab.id
    );
    assert!(harness.state().state.has_unresolved_query_design_edits());
    assert!(!harness.state().state.can_restart_for_update());

    close_event(&mut harness);
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        query_tab.id
    );
    harness.get_by_value("Chosen: IIf(");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::W);
    harness.run_steps(2);
    assert!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == query_tab.id)
    );
    assert!(harness.state().state.has_unresolved_query_design_edits());

    let next = directory.path().join("other.accdb");
    std::fs::write(&next, &original).unwrap();
    let generation = harness.state().generation;
    // NO_CLONE: File-dialog completion consumes its path while the test retains
    // the same selected file for an explicit later retry after Cancel.
    harness.state_mut().file_selected(Some(next.clone()));
    assert_eq!(harness.state().generation, generation);
    assert_eq!(harness.state().state.path, path.to_string_lossy());
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        query_tab.id
    );
    harness.get_by_value("Chosen: IIf(");

    // A queued user transition must recheck the current draft on consumption.
    // This is a plain host open request, never a native review/save capability.
    harness.state_mut().deferred_draft_action =
        Some(DeferredDraftAction::Open(Box::new(Command::Open {
            path: directory.path().join("other.accdb"),
            password: String::new(),
        })));
    // NO_CLONE: The deferred-transition call needs a context handle while the
    // harness is mutably borrowed; egui shares the UI context without payload copies.
    let context = harness.ctx.clone();
    harness.state_mut().resume_table_draft_action(&context);
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(harness.state().generation, generation);
    assert!(harness.state().state.has_unresolved_query_design_edits());
    assert_eq!(std::fs::read(&path).unwrap(), original);

    harness
        .get_all_by_role_and_label(Role::TextInput, "Field expression")
        .nth(1)
        .unwrap()
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().state.has_unresolved_query_design_edits());

    harness.state_mut().file_selected(Some(next));
    finish_reads(&mut harness);
    assert!(harness.state().generation > generation);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[test]
fn acknowledged_same_document_cell_save_preserves_parked_raw_alias_after_fresh_reopen() {
    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    scratch(&mut harness);
    sql(&mut harness, "SELECT Id,Score FROM Items");
    run(&mut harness);
    design(&mut harness);
    let original_tab = harness.state().state.active_object().unwrap();
    field(&mut harness, 1, "Chosen: IIf(");
    show_items(&mut harness);
    assert!(harness.state().state.has_unresolved_query_design_edits());
    let expected = mutation_fixture::with_rows(&[(1, 42), (2, 20), (3, 30)]);
    cell_edit::begin_replacement(&mut harness, 0, "Score", false, Some("42"));
    cell_edit::review_and_save(&mut harness, &path, &original, &expected);
    assert!(harness.state().state.has_unresolved_query_design_edits());
    assert!(!harness.state().state.native_source_stale());
    assert!(!harness.state().state.can_activate_object(&original_tab));
    assert!(
        harness
            .state_mut()
            .state
            .reveal_unresolved_query_design_edits()
    );
    harness.run_steps(2);
    let fresh = harness.state().state.active_object().unwrap();
    assert_eq!(fresh.id, original_tab.id);
    assert_ne!(
        fresh.target.document_revision,
        original_tab.target.document_revision
    );
    harness.get_by_value("Chosen: IIf(");
    assert!(harness.state().state.query_data.is_none());
    assert!(harness.state().state.query_design_request_id().is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
}
