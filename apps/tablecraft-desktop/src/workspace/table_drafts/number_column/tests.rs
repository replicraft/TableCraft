//! Exact Save/Discard/Cancel choices compose real native review and checked reopen.
use crate::workspace::{
    Desktop,
    test_support::{self, finish_reads},
};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;
use tablecraft_ui::{ObjectKind, ObjectTarget};

fn click(harness: &mut Harness<'_, Desktop>, label: &str) {
    harness
        .get_all_by_role_and_label(Role::Button, label)
        .find(|node| !node.is_disabled())
        .expect("enabled modal choice")
        .click_accesskit();
    harness.run_steps(1);
}

fn wait_number(harness: &mut Harness<'_, Desktop>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.has_save() {
        assert!(
            Instant::now() < deadline,
            "Number review/save/read did not settle"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
}

fn name(harness: &mut Harness<'_, Desktop>, field: &str) -> tablecraft_ui::ObjectTab {
    harness.state_mut().state.number_column_available = true;
    harness.state_mut().inspect_number_column_availability();
    finish_reads(harness);
    let tab = harness.state().state.active_object().unwrap();
    assert!(
        harness
            .state_mut()
            .state
            .begin_number_column_name(&tab, field.into())
    );
    tab
}

#[test]
fn close_cancel_preserves_name_then_discard_closes_without_native_mutation() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let tab = name(&mut harness, "Score");
    harness.state_mut().close_object(tab.clone());
    harness.run_steps(2);
    assert!(harness.state().state.number_column_close_open());
    assert!(harness.state().deferred_draft_action.is_some());
    click(&mut harness, "Cancel");
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(
        harness.state().state.number_column_name().unwrap().1,
        "Score"
    );
    assert!(harness.state().state.object_tabs().contains(&tab));
    harness.state_mut().close_object(tab.clone());
    harness.run_steps(2);
    click(&mut harness, "Don't Save");
    assert!(!harness.state().state.number_column_open());
    assert!(!harness.state().state.object_tabs().contains(&tab));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
}

#[test]
fn close_save_waits_for_checked_after_then_closes_preserved_id_and_keeps_scratch() {
    let before = raw::source(raw::Source::IdOnly);
    let after = raw::append(&before, raw::Source::IdOnly, "Score");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    harness.state_mut().state.number_column_available = true;
    harness.state_mut().inspect_number_column_availability();
    finish_reads(&mut harness);
    let table = harness.state().state.active_object().unwrap();
    let scratch = harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: table.target.document_revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    let exact = " SELECT 9 AS [Kept]; -- untouched scratch\n";
    assert!(harness.state_mut().state.replace_query_sql(exact.into()));
    harness.state_mut().activate_object(table.clone());
    finish_reads(&mut harness);
    assert!(
        harness
            .state_mut()
            .state
            .begin_number_column_name(&table, "Score".into())
    );
    harness.state_mut().close_object(table.clone());
    harness.run_steps(2);
    click(&mut harness, "Save");
    wait_number(&mut harness);
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().state.number_column_open());
    let tabs = harness.state().state.object_tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].id, scratch.id);
    assert_eq!(tabs[0].target.kind, ObjectKind::Sql);
    assert_ne!(
        tabs[0].target.document_revision,
        scratch.target.document_revision
    );
    assert_eq!(harness.state().state.sql, exact);
}

#[test]
fn refused_named_review_keeps_close_intent_and_exact_name_until_explicit_cancel() {
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let tab = name(&mut harness, "ID");
    harness.state_mut().close_object(tab.clone());
    harness.run_steps(2);
    click(&mut harness, "Save");
    wait_number(&mut harness);
    assert!(harness.state().state.number_column_close_open());
    assert!(harness.state().deferred_draft_action.is_some());
    assert!(harness.state().state.error.is_some());
    assert_eq!(harness.state().state.number_column_name().unwrap().1, "ID");
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    click(&mut harness, "Cancel");
    assert!(harness.state().deferred_draft_action.is_none());
    assert!(harness.state().state.can_edit_number_column_name(&tab));
    assert_eq!(harness.state().state.number_column_name().unwrap().1, "ID");
}

#[test]
fn deferred_file_open_cancel_preserves_exact_request_and_number_name() {
    use crate::workspace::table_drafts::DeferredDraftAction;
    use tablecraft_runtime::Command;
    let before = raw::source(raw::Source::IdOnly);
    let (directory, path, _, mut harness) = test_support::opened(before.clone());
    let tab = name(&mut harness, "Score");
    let generation = harness.state().generation;
    let next = directory.path().join("other.accdb");
    std::fs::write(&next, &before).unwrap();
    harness.state_mut().open_request(Command::Open {
        path: next.clone(),
        password: String::new(),
    });
    harness.state_mut().open_request(Command::Open {
        path: directory.path().join("never-open.accdb"),
        password: String::new(),
    });
    assert_eq!(harness.state().generation, generation);
    assert!(
        matches!(&harness.state().deferred_draft_action, Some(DeferredDraftAction::Open(command)) if matches!(command.as_ref(),Command::Open {path,..} if path==&next))
    );
    harness.run_steps(2);
    click(&mut harness, "Cancel");
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(
        harness.state().state.number_column_name().unwrap().1,
        "Score"
    );
    assert_eq!(harness.state().state.active_object(), Some(tab));
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn headless_window_close_cancel_preserves_name_and_discard_emits_close_without_writing() {
    use crate::workspace::table_drafts::DeferredDraftAction;
    let before = raw::source(raw::Source::IdOnly);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    name(&mut harness, "Score");
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
    assert!(matches!(
        harness.state().deferred_draft_action,
        Some(DeferredDraftAction::CloseWindow)
    ));
    harness.run_steps(2);
    click(&mut harness, "Cancel");
    assert!(harness.state().deferred_draft_action.is_none());
    assert_eq!(
        harness.state().state.number_column_name().unwrap().1,
        "Score"
    );
    harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    harness.run_steps(2);
    click(&mut harness, "Don't Save");
    assert!(!harness.state().state.number_column_open());
    assert!(
        harness.output().viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Close)
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
