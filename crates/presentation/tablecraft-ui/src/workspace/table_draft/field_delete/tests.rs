use crate::workspace::{
    Action, EditInput, InsertInput, State,
    preview::{Preview, Scene},
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::{DraftFieldId, DraftFieldType};

fn harness() -> Harness<'static, (State, Vec<Action>)> {
    let mut preview = Preview::new(Scene::InitialTableDraft);
    let mut state = std::mem::take(preview.state_mut());
    state.table_save_available = true;
    for kind in [DraftFieldType::ShortText, DraftFieldType::LongInteger] {
        assert!(state.add_table_draft_field(kind));
        assert!(state.table_draft.as_mut().unwrap().commit_rename());
    }
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1280.0, 820.0))
        .build_ui_state(
            |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
            (state, Vec::new()),
        );
    crate::configure(&harness.ctx);
    harness.run_steps(3);
    harness
}

fn ids(harness: &Harness<'_, (State, Vec<Action>)>) -> Vec<DraftFieldId> {
    harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .fields()
        .iter()
        .map(|field| field.id())
        .collect()
}

fn set_input(state: &mut State, id: DraftFieldId, text: &str, is_null: bool) {
    assert!(state.table_draft.as_mut().unwrap().record.set_input(
        id,
        InsertInput::Value(EditInput {
            text: text.into(),
            is_null
        }),
    ));
}

#[test]
fn context_menu_supports_pointer_keyboard_and_accessibility_without_primary_activation() {
    let mut harness = harness();
    harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .click();
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete Field")
            .is_none()
    );
    harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .click_secondary();
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Rename Field")
            .is_some()
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Delete Field")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_field_delete_open());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .is_focused()
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.table_draft_field_delete_open());

    let target_node = harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .accesskit_node()
        .locate()
        .0;
    harness.event(egui::Event::AccessKitActionRequest(
        egui::accesskit::ActionRequest {
            action: egui::accesskit::Action::ShowContextMenu,
            target_tree: egui::accesskit::TreeId::ROOT,
            target_node,
            data: None,
        },
    ));
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete Field")
            .is_some()
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn cancel_keeps_exact_hidden_input_and_confirm_removes_only_the_selected_identity() {
    let mut harness = harness();
    let fields = ids(&harness);
    let state = &mut harness.state_mut().0;
    set_input(state, fields[1], "Raw hidden Ω input", false);
    assert!(
        state
            .table_draft
            .as_mut()
            .unwrap()
            .record
            .set_input(fields[1], InsertInput::Omitted)
    );
    set_input(state, fields[2], "0007", false);
    let schema_revision = state.table_draft_revision().unwrap();
    let record_revision = state.table_draft.as_ref().unwrap().record_revision();
    assert!(state.request_table_draft_field_delete(fields[1]));
    assert!(!state.add_table_draft_field(DraftFieldType::Currency));
    assert!(!state.request_table_draft_save());
    assert!(!state.request_table_draft_discard());
    let tab = state.active_object().unwrap();
    assert!(!state.close_object(&tab));
    assert!(state.cancel_table_draft_field_delete());
    let editor = state.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.revision(), schema_revision);
    assert_eq!(editor.record_revision(), record_revision);
    let snapshot = editor.draft.snapshot("Items").unwrap();
    assert_eq!(
        editor.record.raw_texts(&snapshot).unwrap(),
        ["", "Raw hidden Ω input", "0007"]
    );
    assert!(matches!(
        editor.record.inputs(&snapshot).unwrap()[1],
        InsertInput::Omitted
    ));

    assert!(state.request_table_draft_field_delete(fields[1]));
    assert!(state.confirm_table_draft_field_delete());
    let editor = state.table_draft.as_ref().unwrap();
    assert_eq!(
        editor
            .draft
            .fields()
            .iter()
            .map(|field| field.id())
            .collect::<Vec<_>>(),
        [fields[0], fields[2]]
    );
    let snapshot = editor.draft.snapshot("Items").unwrap();
    assert_eq!(editor.record.raw_texts(&snapshot).unwrap(), ["", "0007"]);
    assert_eq!(editor.draft.revision(), schema_revision + 1);
    assert_eq!(editor.record_revision(), record_revision + 1);
    assert!(state.database.as_ref().unwrap().tables.is_empty());
    assert!(harness.state().1.is_empty());
}

#[test]
fn stale_schema_record_document_or_missing_field_confirmation_never_deletes() {
    for changed in 0..4 {
        let mut harness = harness();
        let fields = ids(&harness);
        let state = &mut harness.state_mut().0;
        assert!(state.request_table_draft_field_delete(fields[1]));
        match changed {
            0 => state
                .table_draft
                .as_mut()
                .unwrap()
                .draft
                .rename_field(fields[1], "Changed")
                .unwrap(),
            1 => set_input(state, fields[2], "latest invalid text", false),
            2 => state.database.as_mut().unwrap().document_revision += 1,
            _ => state
                .table_draft
                .as_mut()
                .unwrap()
                .draft
                .remove_field(fields[1])
                .unwrap(),
        }
        let schema_revision = state.table_draft_revision();
        let record_revision = state.table_draft.as_ref().unwrap().record_revision();
        assert!(!state.confirm_table_draft_field_delete());
        assert_eq!(state.table_draft_revision(), schema_revision);
        assert_eq!(
            state.table_draft.as_ref().unwrap().record_revision(),
            record_revision
        );
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn primary_and_stored_fields_are_protected_and_last_user_field_removes_its_pending_record() {
    let mut harness = harness();
    let fields = ids(&harness);
    let state = &mut harness.state_mut().0;
    assert!(!state.request_table_draft_field_delete(fields[0]));
    set_input(state, fields[1], "Ada", false);
    for field in &fields[1..] {
        assert!(state.request_table_draft_field_delete(*field));
        assert!(state.confirm_table_draft_field_delete());
    }
    let editor = state.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields().len(), 1);
    assert!(editor.draft.fields()[0].is_primary());
    assert!(editor.draft.is_modified());
    assert!(!editor.record.is_modified());
    assert!(harness.state().1.is_empty());
    let mut stored = Preview::new(Scene::Datasheet);
    assert!(
        !stored
            .state_mut()
            .request_table_draft_field_delete(fields[1])
    );
}

#[test]
fn rename_context_command_enters_the_existing_inline_editor_without_losing_record_input() {
    let mut harness = harness();
    let fields = ids(&harness);
    set_input(&mut harness.state_mut().0, fields[1], "Ada", false);
    harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .click_secondary();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Rename Field")
        .click_accesskit();
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field name")
            .is_focused()
    );
    harness.event(egui::Event::Text("Name".into()));
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields()[1].id(), fields[1]);
    assert_eq!(editor.draft.fields()[1].name(), "Name");
    let snapshot = editor.draft.snapshot("Items").unwrap();
    assert_eq!(editor.record.raw_texts(&snapshot).unwrap()[1], "Ada");
    assert!(harness.state().1.is_empty());
}

#[test]
fn compact_confirmation_keeps_full_name_and_both_buttons_in_all_languages_and_themes() {
    for locale in crate::Locale::ALL {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            let name = "界".repeat(64);
            let mut prepared = harness();
            let fields = ids(&prepared);
            let state = &mut prepared.state_mut().0;
            state.localizer = crate::Localizer::new(locale).unwrap();
            state.theme = theme;
            state
                .table_draft
                .as_mut()
                .unwrap()
                .draft
                .rename_field(fields[1], &name)
                .unwrap();
            assert!(state.request_table_draft_field_delete(fields[1]));
            let mut args = crate::FluentArgs::new();
            args.set("field", name);
            let question = state
                .localizer
                .format("table-draft-delete-field-question", &args);
            let cancel = state.localizer.text("table-draft-cancel");
            let confirm = state.localizer.text("table-draft-delete-field-confirm");
            let state = std::mem::take(state);
            let mut compact = Harness::builder()
                .with_size(egui::vec2(320.0, 420.0))
                .build_ui_state(
                    |ui, state: &mut State| {
                        crate::localization::install(ui.ctx(), &state.localizer);
                        crate::theme::set_theme(ui.ctx(), state.theme);
                        crate::fonts::configure(ui.ctx(), state.localizer.locale());
                        state.table_draft_field_delete_dialog(ui.ctx());
                    },
                    state,
                );
            crate::configure(&compact.ctx);
            compact.run_steps(3);
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(320.0, 420.0));
            for node in [
                compact.get_by_label(&question),
                compact.get_by_role_and_label(Role::Button, &cancel),
                compact.get_by_role_and_label(Role::Button, &confirm),
            ] {
                assert!(
                    viewport.contains_rect(node.rect()),
                    "{locale:?} {theme:?}: {:?}",
                    node.rect()
                );
            }
            assert!(
                compact
                    .get_by_role_and_label(Role::Button, &cancel)
                    .is_focused()
            );
            assert_eq!(
                compact
                    .state()
                    .table_draft
                    .as_ref()
                    .unwrap()
                    .draft
                    .fields()
                    .len(),
                3
            );
            assert!(compact.state().database.as_ref().unwrap().tables.is_empty());
        }
    }
}

#[test]
fn a_parked_drafts_popup_does_not_transfer_to_the_next_draft_with_reused_field_ids() {
    let mut harness = harness();
    let previous = harness.state().0.active_object().unwrap();
    let previous_header = harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .accesskit_node()
        .locate()
        .0;
    harness
        .get_by_role_and_label(Role::ColumnHeader, "Field1")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete Field")
            .is_some()
    );
    let revision = previous.target.document_revision;
    assert!(
        harness
            .state_mut()
            .0
            .prepare_object_open(crate::ObjectTarget {
                document_revision: revision,
                kind: crate::ObjectKind::Sql,
            })
            .is_some()
    );
    assert!(harness.state_mut().0.close_object_confirmed(&previous));
    assert!(harness.state_mut().0.open_initial_table_draft());
    assert!(
        harness
            .state_mut()
            .0
            .add_table_draft_field(DraftFieldType::ShortText)
    );
    assert!(
        harness
            .state_mut()
            .0
            .table_draft
            .as_mut()
            .unwrap()
            .commit_rename()
    );
    harness.run_steps(3);
    assert_ne!(
        harness
            .get_by_role_and_label(Role::ColumnHeader, "Field1")
            .accesskit_node()
            .locate()
            .0,
        previous_header
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Delete Field")
            .is_none()
    );
    assert!(harness.state().1.is_empty());
}
