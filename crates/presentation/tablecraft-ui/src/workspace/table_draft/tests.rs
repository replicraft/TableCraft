use super::*;
use crate::{Database, ObjectTarget};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

fn state() -> State {
    let mut state = State {
        database: Some(Database {
            document_revision: 71,
            title: "New database".into(),
            title_path: None,
            format: "Access 2016".into(),
            tables: vec![],
            queries: vec![],
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    state
}

fn harness() -> Harness<'static, (State, Vec<Action>)> {
    let mut state = state();
    assert!(state.open_initial_table_draft());
    let harness = Harness::builder().build_ui_state(
        |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
        (state, Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness
}

#[test]
fn explicit_initial_draft_never_invents_catalog_rows_or_native_loads() {
    let mut state = state();
    assert!(!state.has_unsaved_table_drafts());
    assert!(state.open_initial_table_draft());
    assert!(!state.open_initial_table_draft());
    assert!(state.database.as_ref().unwrap().tables.is_empty());
    assert!(state.data.is_none());
    let tab = state.active_object().unwrap();
    assert!(matches!(tab.target.kind, ObjectKind::DraftTable { .. }));
    assert!(!state.active_object_load().unwrap().needs_load);
    assert!(state.can_view_object_design(&tab));
    assert!(!state.has_modified_table_drafts());
    assert!(state.close_object(&tab));
    assert!(!state.has_unsaved_table_drafts());
}

#[test]
fn dirty_close_preserves_draft_and_busy_mutation_refuses() {
    let mut state = state();
    assert!(state.open_initial_table_draft());
    state.busy = true;
    assert!(!state.add_table_draft_field(DraftFieldType::ShortText));
    assert_eq!(state.table_draft_revision(), Some(0));
    state.busy = false;
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let tab = state.active_object().unwrap();
    assert!(!state.close_object(&tab));
    assert!(state.has_modified_table_drafts());
    assert_eq!(state.active_object(), Some(tab));
    assert!(state.table_draft_close.is_some());
}

#[test]
fn add_header_has_no_native_selection_and_discard_cancel_is_explicit() {
    let mut harness = harness();
    harness.run();
    let add = harness.get_by_role_and_label(Role::Button, "Click to Add");
    assert!(add.is_focused());
    add.click();
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Short Text (255)")
        .click();
    harness.run();
    assert!(harness.state().0.has_modified_table_drafts());
    assert!(harness.state().0.table_selection.position().is_none());
    assert!(harness.state_mut().0.request_table_draft_discard());
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run();
    assert!(harness.state().0.has_modified_table_drafts());
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::TableDraftDiscardCancelled))
    );
    assert!(harness.state_mut().0.request_table_draft_discard());
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Discard")
        .click();
    harness.run();
    assert!(!harness.state().0.has_unsaved_table_drafts());
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::TableDraftsDiscarded))
    );
}

#[test]
fn named_field_input_rejects_oversized_replacement_without_renaming_a_prefix() {
    let mut harness = harness();
    assert!(
        harness
            .state_mut()
            .0
            .add_table_draft_field(DraftFieldType::ShortText)
    );
    harness.run();
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .input_mut()
        .events
        .push(egui::Event::Paste("A".repeat(65)));
    harness.run();

    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.rename.as_ref().unwrap().name, "Field1");
    assert_eq!(editor.draft.fields()[1].name(), "Field1");
    assert_eq!(editor.draft.revision(), 1);
    assert!(editor.error);
}

#[test]
fn parked_draft_stays_modified_and_layout_cannot_replace_it() {
    let mut state = state();
    assert!(state.open_initial_table_draft());
    assert!(state.add_table_draft_field(DraftFieldType::Number));
    let draft = state.active_object().unwrap();
    let sql = ObjectTarget {
        document_revision: 71,
        kind: ObjectKind::Sql,
    };
    assert!(state.prepare_object_open(sql).is_some());
    assert!(state.table_draft.is_none());
    assert!(state.has_modified_table_drafts());
    let layout = state.capture_workspace_layout();
    assert!(!state.restore_workspace_layout(layout, ObjectKind::Sql));
    assert!(state.has_modified_table_drafts());
    assert!(state.activate_object(&draft));
    assert_eq!(state.table_draft_revision(), Some(1));
    assert!(state.data.is_none());
    assert!(!state.can_copy_selection());
}

#[test]
fn existing_individual_close_does_not_accept_a_global_deferred_transition() {
    let mut state = state();
    assert!(state.open_initial_table_draft());
    assert!(state.add_table_draft_field(DraftFieldType::Number));
    let draft = state.active_object().unwrap();
    assert!(!state.close_object(&draft));
    assert!(!state.request_table_draft_discard());
    assert!(state.has_modified_table_drafts());
    assert!(matches!(state.table_draft_close, Some(CloseIntent::Tab(_))));
}

#[test]
fn stale_discard_approval_cannot_close_a_newer_draft_revision() {
    let mut state = state();
    assert!(state.open_initial_table_draft());
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let tab = state.active_object().unwrap();
    state.table_draft_discard_approved = Some((
        tab.clone(),
        1,
        state.table_draft_record_revision_for(&tab).unwrap(),
        state.table_draft_property_revision_for(&tab).unwrap(),
    ));
    assert!(state.add_table_draft_field(DraftFieldType::Number));
    assert!(!state.close_object(&tab));
    assert_eq!(state.table_draft_revision(), Some(2));
    assert!(state.table_draft_discard_approved.is_none());
    assert!(state.has_modified_table_drafts());
}

#[test]
fn initial_record_is_raw_widget_input_and_name_dialog_cancel_preserves_it() {
    let mut harness = harness();
    harness.state_mut().0.table_save_available = true;
    assert!(
        harness
            .state_mut()
            .0
            .add_table_draft_field(DraftFieldType::ShortText)
    );
    harness.run_steps(2);
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("Customer".into()));
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(3);
    let mut args = fluent_bundle::FluentArgs::new();
    args.set("field", "Customer");
    let input_label = harness
        .state()
        .0
        .localizer
        .format("table-draft-record-field", &args);
    harness
        .get_by_role_and_label(Role::TextInput, &input_label)
        .focus();
    harness.run_steps(1);
    harness
        .input_mut()
        .events
        .push(egui::Event::Text("  Ada  ".into()));
    harness.run_steps(2);
    let original = vec![
        crate::InsertInput::Omitted,
        crate::InsertInput::Value(crate::EditInput {
            text: "  Ada  ".into(),
            is_null: false,
        }),
    ];
    {
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        assert_eq!(
            editor
                .record
                .inputs(&editor.draft.snapshot("Table1").unwrap())
                .unwrap(),
            original
        );
        assert!(editor.record.is_modified());
    }
    assert!(harness.state_mut().0.request_table_draft_save());
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(
        editor
            .record
            .inputs(&editor.draft.snapshot("Table1").unwrap())
            .unwrap(),
        original
    );
    assert!(harness.state().0.has_modified_table_drafts());
    assert!(harness.state().1.is_empty());
}

#[test]
fn stale_discard_approval_cannot_close_new_record_input_with_unchanged_schema() {
    let mut state = state();
    assert!(state.open_initial_table_draft());
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let tab = state.active_object().unwrap();
    let schema_revision = state.table_draft_revision_for(&tab).unwrap();
    let record_revision = state.table_draft_record_revision_for(&tab).unwrap();
    state.table_draft_discard_approved = Some((
        tab.clone(),
        schema_revision,
        record_revision,
        state.table_draft_property_revision_for(&tab).unwrap(),
    ));
    let editor = state.table_draft.as_mut().unwrap();
    let field = editor.draft.fields()[1].id();
    assert!(editor.record.set_input(
        field,
        crate::InsertInput::Value(crate::EditInput {
            text: "Do not discard".into(),
            is_null: false,
        })
    ));
    assert_eq!(state.table_draft_revision_for(&tab), Some(schema_revision));
    assert!(!state.close_object(&tab));
    assert!(state.table_draft_discard_approved.is_none());
    assert!(state.has_unsaved_table_drafts());
}
