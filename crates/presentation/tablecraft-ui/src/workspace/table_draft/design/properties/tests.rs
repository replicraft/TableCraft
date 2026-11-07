use crate::workspace::{
    Action, EditInput, InitialRecord, InsertInput, State,
    preview::{Preview, Scene},
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::{DraftDefaultMode, DraftFieldId, DraftFieldType};

fn state() -> (State, DraftFieldId) {
    let mut preview = Preview::new(Scene::InitialTableDraft);
    let mut state = std::mem::take(preview.state_mut());
    state.table_save_available = true;
    assert!(state.add_table_draft_field(DraftFieldType::ShortText));
    assert!(state.table_draft.as_mut().unwrap().commit_rename());
    let field = state.table_draft.as_ref().unwrap().draft.fields()[1].id();
    assert!(state.show_table_draft_design());
    state.table_draft.as_mut().unwrap().design.selected = Some(field);
    (state, field)
}

fn harness(state: State) -> Harness<'static, (State, Vec<Action>)> {
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

fn replace(harness: &mut Harness<'_, (State, Vec<Action>)>, role: Role, label: &str, value: &str) {
    harness.get_by_role_and_label(role, label).focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(egui::Event::Text(value.into()));
    harness.run_steps(2);
}

fn choose(harness: &mut Harness<'_, (State, Vec<Action>)>, label: &str, value: &str) {
    harness
        .get_by_role_and_label(Role::ComboBox, label)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, value)
        .click_accesskit();
    harness.run_steps(2);
}

#[test]
fn rendering_properties_is_not_a_user_change_and_controls_write_exact_typed_choices() {
    let (state, _) = state();
    let mut harness = harness(state);
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .property_revision(),
        0
    );
    replace(&mut harness, Role::TextInput, "Field Size", "0012");
    choose(&mut harness, "Required", "Yes");
    choose(&mut harness, "Allow Zero Length", "No");
    choose(&mut harness, "Default Value", "Literal");
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "  exact Ω text  ",
    );
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    let snapshot = editor.draft.snapshot("Items").unwrap();
    assert_eq!(snapshot.fields()[1].short_text_units(), Some(12));
    assert_eq!(snapshot.fields()[1].required(), Some(true));
    assert_eq!(snapshot.fields()[1].allow_zero_length(), Some(false));
    assert!(
        matches!(snapshot.fields()[1].default_mode(), DraftDefaultMode::Literal(text) if text == "  exact Ω text  ")
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field Size")
            .value()
            .as_deref(),
        Some("0012")
    );
    assert!(harness.state().1.is_empty());
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
}

#[test]
fn invalid_size_survives_view_tab_and_name_changes_until_explicit_correction() {
    let (state, field) = state();
    let mut harness = harness(state);
    replace(&mut harness, Role::TextInput, "Field Size", "not a size");
    assert!(!harness.state_mut().0.request_table_draft_save());
    assert!(
        !harness
            .state_mut()
            .0
            .set_table_draft_field_type(field, DraftFieldType::LongInteger)
    );
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .short_text_units(),
        Some(255)
    );
    assert!(harness.state_mut().0.show_table_draft_datasheet());
    harness.run_steps(2);
    let draft = harness.state().0.active_object().unwrap();
    let scratch = harness
        .state_mut()
        .0
        .prepare_object_open(crate::ObjectTarget {
            document_revision: draft.target.document_revision,
            kind: crate::ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    assert!(harness.state_mut().0.activate_object(&draft));
    assert!(harness.state_mut().0.show_table_draft_design());
    assert!(harness.state().0.object_tabs().contains(&scratch));
    harness
        .state_mut()
        .0
        .table_draft
        .as_mut()
        .unwrap()
        .draft
        .rename_field(field, "Renamed")
        .unwrap();
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field Size")
            .value()
            .as_deref(),
        Some("not a size")
    );
    replace(&mut harness, Role::TextInput, "Field Size", "24");
    assert!(harness.state_mut().0.request_table_draft_save());
    assert!(harness.state().1.is_empty());
}

#[test]
fn default_modes_keep_hidden_literal_text_and_multiline_input_never_becomes_a_formula() {
    let (state, _) = state();
    let mut harness = harness(state);
    choose(&mut harness, "Default Value", "Literal");
    let raw = "=Now()\nraw second line";
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        raw,
    );
    choose(&mut harness, "Default Value", "No default");
    assert!(matches!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .default_mode(),
        DraftDefaultMode::Absent
    ));
    choose(&mut harness, "Default Value", "Literal");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Literal default")
            .value()
            .as_deref(),
        Some(raw)
    );
    choose(&mut harness, "Default Value", "NULL");
    assert!(matches!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .default_mode(),
        DraftDefaultMode::Null
    ));
    choose(&mut harness, "Default Value", "Type default");
    assert!(matches!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[1]
            .default_mode(),
        DraftDefaultMode::TypeDefault
    ));
    assert!(harness.state().1.is_empty());
}

#[test]
fn oversized_default_paste_keeps_previous_text_and_blocks_save_until_corrected() {
    let (state, _) = state();
    let mut harness = harness(state);
    choose(&mut harness, "Default Value", "Literal");
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "kept",
    );
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        &"x".repeat(4097),
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Literal default")
            .value()
            .as_deref(),
        Some("kept")
    );
    assert!(!harness.state_mut().0.request_table_draft_save());
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "fixed",
    );
    assert!(harness.state_mut().0.request_table_draft_save());
    assert!(harness.state().1.is_empty());
}

#[test]
fn raw_property_change_invalidates_delete_and_discard_witnesses_without_model_change() {
    let (mut state, field) = state();
    let editor = state.table_draft.as_mut().unwrap();
    let position = editor.properties.ensure(&editor.draft, field).unwrap();
    let schema_revision = editor.draft.revision();
    assert!(state.request_table_draft_field_delete(field));
    let editor = state.table_draft.as_mut().unwrap();
    assert!(
        !editor
            .properties
            .set_size(&mut editor.draft, position, "invalid".into(), false)
    );
    assert_eq!(editor.draft.revision(), schema_revision);
    assert!(!state.confirm_table_draft_field_delete());
    assert_eq!(state.table_draft.as_ref().unwrap().draft.fields().len(), 2);
    let tab = state.active_object().unwrap();
    state.table_draft_discard_approved = Some((
        tab.clone(),
        schema_revision,
        state.table_draft_record_revision_for(&tab).unwrap(),
        state.table_draft_property_revision_for(&tab).unwrap(),
    ));
    let editor = state.table_draft.as_mut().unwrap();
    assert!(!editor.properties.set_size(
        &mut editor.draft,
        position,
        "new invalid input".into(),
        false
    ));
    assert!(!state.close_object(&tab));
    assert!(state.has_unsaved_table_drafts());
    assert!(state.table_draft_discard_approved.is_none());
}

#[test]
fn property_default_heap_is_included_in_initial_record_retention() {
    let (mut state, field) = state();
    let make = |state: &State| InitialRecord {
        snapshot: state
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .snapshot("Items")
            .unwrap(),
        inputs: vec![
            InsertInput::Omitted,
            InsertInput::Value(EditInput {
                text: "kept".into(),
                is_null: false,
            }),
        ],
        raw_text: vec![String::new(), "kept".into()],
    };
    let before = make(&state).retained_bytes().unwrap();
    let raw = "x".repeat(4096);
    state
        .table_draft
        .as_mut()
        .unwrap()
        .draft
        .set_field_default(field, DraftDefaultMode::Literal(raw))
        .unwrap();
    let after = make(&state).retained_bytes().unwrap();
    assert!(after >= before + 4096);
}

#[test]
fn compact_property_controls_remain_accessible_across_all_languages_and_themes() {
    for locale in crate::Locale::ALL {
        for theme in [crate::Theme::Light, crate::Theme::Dark] {
            let (mut state, field) = state();
            state
                .table_draft
                .as_mut()
                .unwrap()
                .draft
                .set_field_default(field, DraftDefaultMode::Literal("Guest".into()))
                .unwrap();
            state.localizer = crate::Localizer::new(locale).unwrap();
            state.theme = theme;
            let size = state.localizer.text("design-field-size");
            let required = state.localizer.text("design-required");
            let allow_zero_length = state.localizer.text("design-allow-zero-length");
            let default = state.localizer.text("design-default-value");
            let literal = state.localizer.text("table-draft-property-literal-input");
            let general = state.localizer.text("design-general");
            let footer = state.localizer.text("table-draft-design-input-help");
            let mut harness = Harness::builder()
                .with_size(egui::vec2(900.0, 580.0))
                .build_ui_state(
                    |ui, state: &mut (State, Vec<Action>)| state.1.extend(state.0.show(ui)),
                    (state, Vec::new()),
                );
            crate::configure(&harness.ctx);
            harness.run_steps(3);
            let viewport = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 580.0));
            let general_rect = harness.get_by_label(&general).rect();
            let footer_rect = harness.get_by_label(&footer).rect();
            for node in [
                harness.get_by_role_and_label(Role::TextInput, &size),
                harness.get_by_role_and_label(Role::ComboBox, &required),
                harness.get_by_role_and_label(Role::ComboBox, &allow_zero_length),
                harness.get_by_role_and_label(Role::ComboBox, &default),
                harness.get_by_role_and_label(Role::MultilineTextInput, &literal),
            ] {
                let rect = node.rect();
                assert!(
                    viewport.contains_rect(rect),
                    "{locale:?} {theme:?}: {:?}",
                    rect
                );
                assert!(
                    rect.top() >= general_rect.bottom() && rect.bottom() <= footer_rect.top(),
                    "{locale:?} {theme:?}: control {rect:?} must fit between General \
                     {general_rect:?} and footer {footer_rect:?}"
                );
            }
            assert!(harness.state().1.is_empty());
        }
    }
}
