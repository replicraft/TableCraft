use super::*;
use crate::workspace::table_draft::CloseIntent;
use crate::{Database, ObjectKind, Table};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::{DraftFieldType, TableDraft};

fn state() -> State {
    let mut state = State {
        table_save_available: true,
        database: Some(database(71, None)),
        ..State::default()
    };
    state.dismiss_backstage();
    assert!(state.open_initial_table_draft());
    state
}

fn database(revision: u64, table: Option<&str>) -> Database {
    Database {
        document_revision: revision,
        title: "Database".into(),
        title_path: None,
        format: "Access 2016".into(),
        queries: Vec::new(),
        tables: table
            .into_iter()
            .map(|name| Table {
                name: name.into(),
                local: true,
                system: false,
                storage: "Local".into(),
            })
            .collect(),
    }
}

fn issue(state: &mut State, name: &str) -> TableSaveRequest {
    if state.table_draft_save.is_none() {
        assert!(state.request_table_draft_save());
    }
    let mut save = state.table_draft_save.take().unwrap();
    save.name = name.into();
    let mut actions = Vec::new();
    state.issue_table_save(&mut save, &mut actions);
    state.table_draft_save = Some(save);
    let Action::SaveTableDraft { request } = actions.pop().unwrap() else {
        panic!("one consuming declaration request")
    };
    assert!(actions.is_empty());
    request
}

#[test]
fn issued_save_binds_owned_declaration_and_each_retry_has_a_fresh_identity() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let request = issue(&mut state, "Contacts");
    assert_eq!(request.tab.target.document_revision, 71);
    assert!(matches!(
        request.tab.target.kind,
        ObjectKind::DraftTable { .. }
    ));
    assert_eq!(request.snapshot.table_name(), "Contacts");
    assert_eq!(request.snapshot.revision(), 1);
    assert_eq!(request.snapshot.fields().len(), 2);
    assert!(state.begin_table_draft_save(&request));
    assert!(!state.begin_table_draft_save(&request));
    assert!(!state.add_table_draft_field(DraftFieldType::LongInteger));
    assert!(!state.fail_table_draft_save(request.id + 1, "stale".into()));
    assert!(state.fail_table_draft_save(request.id, "No write occurred".into()));
    assert!(state.has_modified_table_drafts());
    assert!(!state.table_draft_save_pending());
    let retry = issue(&mut state, "Contacts");
    assert_ne!(request.id, retry.id);
    assert!(!state.cancel_table_draft_save(request.id));
    assert!(state.begin_table_draft_save(&retry));
}

#[test]
fn completion_requires_verified_new_metadata_and_does_not_publish_optimistically() {
    let mut state = state();
    let request = issue(&mut state, "Contacts");
    assert!(state.begin_table_draft_save(&request));
    assert!(state.database.as_ref().unwrap().tables.is_empty());
    assert!(
        state
            .complete_table_draft_save(request.id, database(72, None))
            .is_none()
    );
    assert!(
        state
            .complete_table_draft_save(request.id, database(71, Some("Contacts")))
            .is_none()
    );
    assert!(
        state
            .complete_table_draft_save(request.id + 1, database(72, Some("Contacts")))
            .is_none()
    );
    assert!(state.has_unsaved_table_drafts());
    let completion = state
        .complete_table_draft_save(request.id, database(72, Some("Contacts")))
        .unwrap();
    assert_eq!(completion.target.document_revision, 72);
    assert!(matches!(completion.target.kind, ObjectKind::Table { ref name } if name == "Contacts"));
    assert!(!completion.close_after_save);
    assert!(!completion.resume_transition);
    assert!(!state.has_unsaved_table_drafts());
    assert!(state.data.is_none());
    assert_eq!(state.database.as_ref().unwrap().tables[0].name, "Contacts");
}

#[test]
fn confirmed_cancellation_keeps_name_and_original_global_close_intent() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    assert!(state.request_table_draft_discard());
    let intent = state.table_draft_close.take().unwrap();
    assert!(state.start_table_draft_save(Some(intent)));
    let request = issue(&mut state, "Customers");
    assert!(state.begin_table_draft_save(&request));
    assert!(state.cancel_table_draft_save(request.id));
    let save = state.table_draft_save.as_ref().unwrap();
    assert_eq!(save.name, "Customers");
    assert!(matches!(save.close, Some(CloseIntent::All)));
    assert_eq!(save.phase, Phase::Naming);
    assert!(state.has_modified_table_drafts());
    let retry = issue(&mut state, "Customers");
    assert!(state.begin_table_draft_save(&retry));
    let completion = state
        .complete_table_draft_save(retry.id, database(72, Some("Customers")))
        .unwrap();
    assert!(completion.resume_transition);
}

#[test]
fn uncertain_save_keeps_pending_owner_and_draft_until_guarded_reopen() {
    let mut state = state();
    let request = issue(&mut state, "Contacts");
    assert!(state.begin_table_draft_save(&request));
    assert!(state.retain_table_draft_save(request.id, "Recovery needed".into()));
    assert!(state.table_draft_save_pending());
    assert!(!state.request_table_draft_save());
    assert!(!state.add_table_draft_field(DraftFieldType::ShortText));
    assert!(state.has_unsaved_table_drafts());
    assert!(
        state
            .complete_table_draft_save(request.id, database(72, Some("Contacts")))
            .is_some()
    );
}

#[test]
fn save_name_editor_is_accessible_rejects_oversized_paste_and_confirms_once() {
    let mut state = state();
    assert!(state.request_table_draft_save());
    let mut harness = Harness::builder()
        .with_size(egui::vec2(900.0, 580.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run();
    let name = harness.get_by_role_and_label(Role::TextInput, "Table name");
    assert!(name.is_focused());
    name.focus();
    harness.step();
    harness.event(egui::Event::Paste("界".repeat(1000)));
    harness.run_steps(2);
    assert_eq!(
        harness.state().0.table_draft_save.as_ref().unwrap().name,
        "Table1"
    );
    harness.get_by_role_and_label(Role::Button, "OK").click();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .1
            .iter()
            .filter(|action| matches!(action, Action::SaveTableDraft { .. }))
            .count(),
        1
    );
    assert!(harness.state().0.table_draft_save_pending());
}

#[test]
fn command_save_only_opens_the_naming_dialog_with_an_admitted_host_capability() {
    let mut state = state();
    state.table_save_available = false;
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut State| {
            state.show(ui);
        },
        state,
    );
    crate::configure(&harness.ctx);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run_steps(2);
    assert!(!harness.state().table_draft_save_open());
    harness.state_mut().table_save_available = true;
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run_steps(2);
    assert!(harness.state().table_draft_save_open());
    assert!(!harness.state().table_draft_save_pending());
}

#[test]
fn individual_close_save_only_closes_after_actual_reopen_metadata_arrives() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::LongInteger));
    let tab = state.active_object().unwrap();
    assert!(!state.close_object(&tab));
    let intent = state.table_draft_close.take().unwrap();
    assert!(state.start_table_draft_save(Some(intent)));
    let request = issue(&mut state, "Inventory");
    assert!(state.begin_table_draft_save(&request));
    assert!(state.has_modified_table_drafts());
    let completion = state
        .complete_table_draft_save(request.id, database(72, Some("Inventory")))
        .unwrap();
    assert!(completion.close_after_save);
    assert!(!completion.resume_transition);
    assert!(state.active_object().is_none());
    assert!(!state.has_unsaved_table_drafts());
}

#[test]
fn cancelling_the_name_prompt_restores_the_original_close_choice_and_name() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    assert!(state.request_table_draft_discard());
    let intent = state.table_draft_close.take().unwrap();
    assert!(state.start_table_draft_save(Some(intent)));
    state.table_draft_save.as_mut().unwrap().name = "Customers".into();
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
        (state, Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.table_draft_save_open());
    assert!(matches!(
        harness.state().0.table_draft_close,
        Some(CloseIntent::All)
    ));
    assert!(harness.state().0.has_modified_table_drafts());
    assert_eq!(
        harness.state().0.table_draft.as_ref().unwrap().save_name,
        "Customers"
    );
    assert!(harness.state().1.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run_steps(2);
    assert!(
        harness
            .state()
            .1
            .iter()
            .any(|action| matches!(action, Action::TableDraftDiscardCancelled))
    );
    assert!(harness.state().0.has_modified_table_drafts());
}

#[test]
fn retained_recovery_controls_keep_owner_until_ack_and_never_enable_retry_after_release() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let request = issue(&mut state, "Contacts");
    assert!(state.begin_table_draft_save(&request));
    assert!(state.retain_table_draft_save_status(
        request.id,
        "Recovery required".into(),
        true,
        true,
        false
    ));
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
        (state, Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Recover save")
        .click();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .1
            .iter()
            .filter(
                |action| matches!(action, Action::RecoverTableDraftSave { id } if *id == request.id)
            )
            .count(),
        1
    );
    assert!(harness.state().0.table_draft_save_pending());
    assert!(harness.state().0.has_modified_table_drafts());
    assert!(
        !harness
            .state_mut()
            .0
            .acknowledge_table_draft_save(request.id)
    );
    assert!(harness.state_mut().0.retain_table_draft_save_status(
        request.id,
        "Resources released; bytes unresolved".into(),
        false,
        false,
        true
    ));
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run_steps(2);
    assert!(harness.state().1.iter().any(
        |action| matches!(action, Action::AcknowledgeTableDraftSave { id } if *id == request.id)
    ));
    assert!(harness.state().0.table_draft_save_pending());
    assert!(
        harness
            .state_mut()
            .0
            .acknowledge_table_draft_save(request.id)
    );
    assert!(!harness.state().0.table_draft_save_pending());
    assert!(harness.state().0.has_modified_table_drafts());
    assert!(!harness.state().0.can_save_table_draft());
    assert!(!harness.state_mut().0.request_table_draft_save());
    assert!(
        !harness
            .state_mut()
            .0
            .add_table_draft_field(DraftFieldType::LongInteger)
    );
    assert!(
        harness
            .state()
            .0
            .database
            .as_ref()
            .unwrap()
            .tables
            .is_empty()
    );
    assert!(
        !harness
            .state_mut()
            .0
            .fail_table_draft_save(request.id, "late".into())
    );
}

#[test]
fn substituted_declaration_or_target_cannot_consume_the_original_issued_request() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let request = issue(&mut state, "Contacts");
    let substituted = TableSaveRequest {
        id: request.id,
        record_revision: request.record_revision,
        tab: request.tab.clone(),
        snapshot: TableDraft::initial().snapshot("Contacts").unwrap(),
    };
    assert!(!state.begin_table_draft_save(&substituted));
    assert!(!state.busy);
    assert!(state.table_draft_save.as_ref().unwrap().phase == Phase::Issued);
    let mut other = TableDraft::initial();
    other.add_field(DraftFieldType::LongInteger).unwrap();
    let wrong_type = TableSaveRequest {
        id: request.id,
        record_revision: request.record_revision,
        tab: request.tab.clone(),
        snapshot: other.snapshot("Contacts").unwrap(),
    };
    assert!(!state.begin_table_draft_save(&wrong_type));
    let mut wrong_target = TableSaveRequest {
        id: request.id,
        record_revision: request.record_revision,
        tab: request.tab.clone(),
        snapshot: state
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .snapshot("Contacts")
            .unwrap(),
    };
    wrong_target.tab.target.document_revision += 1;
    assert!(!state.begin_table_draft_save(&wrong_target));
    assert!(!state.busy);
    assert!(state.table_draft_save.as_ref().unwrap().phase == Phase::Issued);
    assert!(state.begin_table_draft_save(&request));
    assert!(!state.begin_table_draft_save(&request));
}

#[test]
fn focused_name_enter_issues_the_exact_request_once_before_focus_can_be_lost() {
    let mut state = state();
    assert!(state.request_table_draft_save());
    let mut harness = Harness::builder().build_ui_state(
        |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
        (state, Vec::new()),
    );
    crate::configure(&harness.ctx);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Table name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Table name")
        .type_text("Contacts");
    harness.run_steps(2);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    let requests: Vec<_> = harness
        .state()
        .1
        .iter()
        .filter_map(|action| match action {
            Action::SaveTableDraft { request } => Some(request),
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].snapshot.table_name(), "Contacts");
    assert_eq!(requests[0].tab.target.document_revision, 71);
    assert!(harness.state().0.table_draft_save_pending());
    assert!(
        harness
            .state()
            .0
            .database
            .as_ref()
            .unwrap()
            .tables
            .is_empty()
    );
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .1
            .iter()
            .filter(|action| matches!(action, Action::SaveTableDraft { .. }))
            .count(),
        1
    );
}

#[test]
fn row_revision_is_bound_to_issued_save_and_terminal_cancellation_retains_exact_input() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let editor = state.table_draft.as_mut().unwrap();
    let field = editor.draft.fields()[1].id();
    let original = crate::InsertInput::Value(crate::EditInput {
        text: "  exact draft text  ".into(),
        is_null: false,
    });
    assert!(editor.record.set_input(field, original.clone()));
    let request = issue(&mut state, "Contacts");
    let mut substituted = TableSaveRequest {
        id: request.id,
        tab: request.tab.clone(),
        snapshot: state
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .snapshot("Contacts")
            .unwrap(),
        record_revision: request.record_revision + 1,
    };
    assert!(!state.begin_table_draft_save(&substituted));
    assert!(!state.busy);
    substituted.record_revision = request.record_revision;
    assert!(state.begin_table_draft_save(&substituted));
    assert!(state.cancel_table_draft_save(request.id));
    let editor = state.table_draft.as_ref().unwrap();
    let snapshot = editor.draft.snapshot("Contacts").unwrap();
    assert_eq!(
        editor.record.inputs(&snapshot).unwrap(),
        vec![crate::InsertInput::Omitted, original]
    );
    assert!(state.has_modified_table_drafts());
}

#[test]
fn successful_schema_completion_retains_one_original_record_before_discarding_editor() {
    let mut state = state();
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    let editor = state.table_draft.as_mut().unwrap();
    let field = editor.draft.fields()[1].id();
    assert!(editor.record.set_input(
        field,
        crate::InsertInput::Value(crate::EditInput {
            text: "Ada".into(),
            is_null: false,
        })
    ));
    let request = issue(&mut state, "Contacts");
    assert!(state.begin_table_draft_save(&request));
    let completion = state
        .complete_table_draft_save(request.id, database(72, Some("Contacts")))
        .unwrap();
    assert!(completion.record_pending);
    assert!(state.initial_record.is_some());
    assert!(!state.has_unsaved_table_drafts());
    assert!(state.data.is_none());
}

#[test]
fn untouched_new_record_placeholder_never_creates_an_empty_record_continuation() {
    let mut state = state();
    let request = issue(&mut state, "Contacts");
    assert!(state.begin_table_draft_save(&request));
    let completion = state
        .complete_table_draft_save(request.id, database(72, Some("Contacts")))
        .unwrap();
    assert!(!completion.record_pending);
    assert!(state.initial_record.is_none());
}
