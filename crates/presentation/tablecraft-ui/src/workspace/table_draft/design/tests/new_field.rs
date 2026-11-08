use super::{harness, state};
use crate::{
    Locale, Localizer, Theme,
    workspace::{Action, EditInput, InsertInput, State},
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::DraftFieldType;

fn enter_name(harness: &mut Harness<'_, (State, Vec<Action>)>, name: &str) {
    let label = harness
        .state()
        .0
        .localizer
        .text("table-draft-design-new-name");
    harness
        .get_by_role_and_label(Role::TextInput, &label)
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, &label)
        .type_text(name);
    harness.run_steps(2);
}

fn choose_type(harness: &mut Harness<'_, (State, Vec<Action>)>, key: &str) {
    let label = harness
        .state()
        .0
        .localizer
        .text("table-draft-design-new-type");
    harness
        .get_by_role_and_label(Role::ComboBox, &label)
        .click_accesskit();
    harness.run_steps(2);
    let kind = harness.state().0.localizer.text(key);
    harness
        .get_by_role_and_label(Role::Button, &kind)
        .click_accesskit();
    harness.run_steps(2);
}

fn commit_name(harness: &mut Harness<'_, (State, Vec<Action>)>) {
    let label = harness
        .state()
        .0
        .localizer
        .text("table-draft-design-new-name");
    harness
        .get_by_role_and_label(Role::TextInput, &label)
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
}

#[test]
fn inline_duplicate_refusal_and_correction_preserve_kind_ids_and_existing_record() {
    let (mut state, original) = state();
    let editor = state.table_draft.as_mut().unwrap();
    assert!(editor.record.set_input(
        original,
        InsertInput::Value(EditInput {
            text: "  retained 日本語  ".into(),
            is_null: false,
        })
    ));
    let before = editor.draft.snapshot("Table1").unwrap();
    let record_revision = editor.record.revision();
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    enter_name(&mut harness, "Field1");
    choose_type(&mut harness, "design-type-number");
    assert!(
        harness
            .get_by_role_and_label(Role::ComboBox, "New field data type")
            .is_focused()
    );
    harness.key_press(Key::Enter);
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(editor.draft.snapshot("Table1").unwrap() == before);
    assert_eq!(editor.record.revision(), record_revision);
    assert_eq!(editor.new_field.kind, DraftFieldType::Number);
    assert_eq!(editor.new_field.name, "Field1");
    assert!(editor.new_field.rejected);

    enter_name(&mut harness, "Score");
    commit_name(&mut harness);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.revision(), before.revision() + 1);
    let field = editor.draft.fields().last().unwrap();
    assert_eq!(field.name(), "Score");
    assert_eq!(field.kind(), DraftFieldType::Number);
    assert_ne!(field.id(), original);
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    let inputs = editor.record.inputs(&snapshot).unwrap();
    assert!(matches!(&inputs[1], InsertInput::Value(input) if input.text == "  retained 日本語  "));
    assert!(matches!(inputs[2], InsertInput::Omitted));
    assert!(harness.state().1.is_empty());
}

#[test]
fn keyboard_tab_commits_type_and_escape_discards_only_pending_row() {
    let (mut state, _) = state();
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    enter_name(&mut harness, "Score");
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    assert!(
        harness
            .get_by_role_and_label(Role::ComboBox, "New field data type")
            .is_focused()
    );
    harness.key_press(Key::Tab);
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields().last().unwrap().name(), "Score");
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    let record_revision = editor.record.revision();
    let raw_revision = editor.property_revision();
    enter_name(&mut harness, "Discarded");
    choose_type(&mut harness, "design-type-currency");
    harness
        .get_by_role_and_label(Role::TextInput, "New field name")
        .focus();
    harness.run_steps(1);
    let name_focused = harness
        .get_by_role_and_label(Role::TextInput, "New field name")
        .is_focused();
    let type_focused = harness
        .get_by_role_and_label(Role::ComboBox, "New field data type")
        .is_focused();
    let popup_open = egui::Popup::is_any_open(&harness.ctx);
    assert!(
        name_focused,
        "type_focused={type_focused} popup_open={popup_open}"
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(
        !editor.new_field.pending(),
        "name={:?} kind={:?} rejected={} before_name_focus={name_focused} before_type_focus={type_focused} before_popup={popup_open} after_popup={}",
        editor.new_field.name,
        editor.new_field.kind,
        editor.new_field.rejected,
        egui::Popup::is_any_open(&harness.ctx)
    );
    assert!(editor.draft.snapshot("Table1").unwrap() == snapshot);
    assert_eq!(editor.record.revision(), record_revision);
    assert!(editor.property_revision() > raw_revision);
}

#[test]
fn save_commits_pending_first_name_and_oversized_paste_blocks_without_truncation() {
    let mut preview = crate::preview::Preview::new(crate::preview::Scene::FirstTableReady);
    let mut state = std::mem::take(preview.state_mut());
    assert!(state.open_table_design_draft());
    let mut harness = harness(state);
    enter_name(&mut harness, "Name");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_save_open());
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields().len(), 1);
    assert_eq!(editor.draft.fields()[0].name(), "Name");
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    enter_name(&mut harness, "Kept");
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    let raw_revision = editor.property_revision();
    harness.event(egui::Event::Paste("x".repeat(65)));
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.new_field.name, "Kept");
    assert!(editor.new_field.rejected);
    assert!(editor.property_revision() > raw_revision);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(2);
    assert!(!harness.state().0.table_draft_save_open());
    assert!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .snapshot("Table1")
            .unwrap()
            == snapshot
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn localized_type_popups_keep_unavailable_native_choices_inert() {
    for locale in Locale::ALL {
        for theme in [Theme::Light, Theme::Dark] {
            let (mut state, _) = state();
            state.localizer = Localizer::new(locale).unwrap();
            state.theme = theme;
            assert!(state.show_table_draft_design());
            let mut harness = harness(state);
            let label = harness
                .state()
                .0
                .localizer
                .text("table-draft-design-new-type");
            harness
                .get_by_role_and_label(Role::ComboBox, &label)
                .click_accesskit();
            harness.run_steps(2);
            let label = harness.state().0.localizer.text("design-type-long-text");
            let unsupported = harness.get_by_role_and_label(Role::Button, &label);
            assert!(unsupported.accesskit_node().is_disabled());
            unsupported.click_accesskit();
            harness.run_steps(2);
            let editor = harness.state().0.table_draft.as_ref().unwrap();
            assert!(!editor.new_field.pending());
            assert_eq!(editor.draft.fields().len(), 2);
            assert!(harness.state().1.is_empty());
            assert_eq!(
                harness.state().0.localizer.diagnostics().english_fallbacks,
                0
            );
        }
    }
}

#[test]
fn invalid_pending_row_survives_view_refusal_and_object_parking() {
    let (mut state, _) = state();
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    enter_name(&mut harness, "Field1");
    choose_type(&mut harness, "design-type-number");
    let tab = harness.state().0.active_object().unwrap();
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    let raw_revision = editor.property_revision();
    assert!(!harness.state_mut().0.show_table_draft_datasheet());
    assert!(harness.state().0.table_draft_design_visible());
    let sql = harness
        .state_mut()
        .0
        .prepare_object_open(crate::ObjectTarget {
            document_revision: tab.target.document_revision,
            kind: crate::ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert_eq!(harness.state().0.active_object(), Some(sql));
    assert!(harness.state_mut().0.activate_object(&tab));
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.new_field.name, "Field1");
    assert_eq!(editor.new_field.kind, DraftFieldType::Number);
    assert_eq!(editor.property_revision(), raw_revision);
    assert!(editor.draft.snapshot("Table1").unwrap() == snapshot);
    assert!(harness.state().1.is_empty());
}

#[test]
fn actual_name_edit_and_escape_invalidate_all_captured_witnesses_without_schema_or_record_change() {
    let (mut state, field) = state();
    assert!(state.table_draft.as_mut().unwrap().record.set_input(
        field,
        InsertInput::Value(EditInput {
            text: "  original 日本語  ".into(),
            is_null: false,
        })
    ));
    assert!(state.show_table_draft_design());
    let tab = state.active_object().unwrap();
    let mut harness = harness(state);
    assert!(
        harness
            .state_mut()
            .0
            .request_table_draft_field_delete(field)
    );
    let deletion = harness.state_mut().0.table_draft_field_delete.take();

    assert!(harness.state_mut().0.request_table_draft_save());
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(2);
    let Action::SaveTableDraft { request } = harness.state_mut().1.pop().unwrap() else {
        panic!("expected the original consuming save witness");
    };
    let saving = harness.state_mut().0.table_draft_save.take();

    assert!(!harness.state_mut().0.close_object(&tab));
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Discard")
        .click_accesskit();
    harness.run_steps(2);
    assert!(matches!(
        harness.state_mut().1.pop(),
        Some(Action::CloseObject { .. })
    ));
    let discarding = harness.state_mut().0.table_draft_discard_approved.take();
    assert!(discarding.is_some());
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    let before = editor.draft.snapshot("Table1").unwrap();
    let inputs = editor.record.inputs(&before).unwrap();
    let record_revision = editor.record.revision();
    let raw_revision = editor.property_revision();

    enter_name(&mut harness, "Cancelled");
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(!editor.new_field.pending());
    assert!(editor.draft.snapshot("Table1").unwrap() == before);
    assert_eq!(editor.record.inputs(&before).unwrap(), inputs);
    assert_eq!(editor.record.revision(), record_revision);
    assert!(editor.property_revision() > raw_revision);

    harness.state_mut().0.table_draft_field_delete = deletion;
    assert!(!harness.state_mut().0.confirm_table_draft_field_delete());
    harness.state_mut().0.table_draft_save = saving;
    assert!(!harness.state_mut().0.begin_table_draft_save(&request));
    harness.state_mut().0.table_draft_save = None;
    harness.state_mut().0.table_draft_discard_approved = discarding;
    assert!(!harness.state_mut().0.close_object(&tab));
    assert!(harness.state().0.object_tabs().contains(&tab));
    assert!(harness.state().1.is_empty());
}

#[test]
fn first_escape_closes_type_popup_and_second_escape_cancels_only_pending_row() {
    let (mut state, field) = state();
    assert!(state.table_draft.as_mut().unwrap().record.set_input(
        field,
        InsertInput::Value(EditInput {
            text: "  original 日本語  ".into(),
            is_null: false,
        })
    ));
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    enter_name(&mut harness, "Pending");
    choose_type(&mut harness, "design-type-currency");
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    let inputs = editor.record.inputs(&snapshot).unwrap();
    let record_revision = editor.record.revision();
    let raw_revision = editor.property_revision();
    harness
        .get_by_role_and_label(Role::ComboBox, "New field data type")
        .click_accesskit();
    harness.run_steps(2);
    assert!(egui::Popup::is_any_open(&harness.ctx));
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!egui::Popup::is_any_open(&harness.ctx));
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.new_field.name, "Pending");
    assert_eq!(editor.new_field.kind, DraftFieldType::Currency);
    assert!(editor.new_field.pending());
    assert!(!editor.new_field.rejected);
    assert!(editor.draft.snapshot("Table1").unwrap() == snapshot);
    assert_eq!(editor.record.inputs(&snapshot).unwrap(), inputs);
    assert_eq!(editor.record.revision(), record_revision);
    assert_eq!(editor.property_revision(), raw_revision);

    harness.key_press(Key::Escape);
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(!editor.new_field.pending());
    assert!(editor.draft.snapshot("Table1").unwrap() == snapshot);
    assert_eq!(editor.record.inputs(&snapshot).unwrap(), inputs);
    assert_eq!(editor.record.revision(), record_revision);
    assert!(editor.property_revision() > raw_revision);
    assert!(harness.state().1.is_empty());
}

#[test]
fn enter_in_open_type_popup_does_not_commit_the_pending_declaration_behind_it() {
    for key in [Key::Enter, Key::Tab] {
        let (mut state, _) = state();
        assert!(state.show_table_draft_design());
        let mut harness = harness(state);
        enter_name(&mut harness, "Pending");
        choose_type(&mut harness, "design-type-currency");
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        let snapshot = editor.draft.snapshot("Table1").unwrap();
        let inputs = editor.record.inputs(&snapshot).unwrap();
        let record_revision = editor.record.revision();
        harness
            .get_by_role_and_label(Role::ComboBox, "New field data type")
            .click_accesskit();
        harness.run_steps(2);
        assert!(egui::Popup::is_any_open(&harness.ctx));
        harness.key_press(key);
        harness.run_steps(2);
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        assert!(
            editor.draft.snapshot("Table1").unwrap() == snapshot,
            "{key:?} belongs to the open dropdown; it must not append the pending field"
        );
        assert_eq!(editor.new_field.name, "Pending");
        assert!(editor.new_field.pending());
        assert_eq!(editor.record.inputs(&snapshot).unwrap(), inputs);
        assert_eq!(editor.record.revision(), record_revision);
        assert!(harness.state().1.is_empty());
    }
}
