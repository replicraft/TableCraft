use crate::{
    Database, Locale, Localizer, Theme,
    workspace::{Action, EditInput, InsertInput, State, ribbon::Tab},
};
use egui::{
    Key,
    accesskit::{Role, Toggled},
};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_builders::table::{DraftFieldId, DraftFieldType};

fn state() -> State {
    let mut state = State {
        table_save_available: true,
        database: Some(Database {
            document_revision: 71,
            title: "Blank database".into(),
            title_path: None,
            format: "Access 2016".into(),
            tables: Vec::new(),
            queries: Vec::new(),
        }),
        ..State::default()
    };
    state.dismiss_backstage();
    state.set_table_creation_available(71, true);
    state.set_table_design_available(71, true);
    state
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

fn add(state: &mut State, kind: DraftFieldType, name: &str) -> DraftFieldId {
    assert!(state.add_table_draft_field(kind));
    let editor = state.table_draft.as_mut().unwrap();
    assert!(editor.commit_rename());
    let id = editor.draft.fields().last().unwrap().id();
    editor.draft.rename_field(id, name).unwrap();
    id
}

fn key_prompt(harness: &mut Harness<'_, (State, Vec<Action>)>) {
    assert!(harness.state_mut().0.request_table_draft_save());
    harness.run_steps(2);
    let label = harness.state().0.localizer.text("table-draft-ok");
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_save_open());
    assert!(!harness.state().0.table_draft_save_pending());
    let title = harness.state().0.localizer.text("table-draft-no-key-title");
    assert!(harness.query_by_label(&title).is_some());
    assert!(harness.state().1.is_empty());
}

#[test]
fn create_design_ribbon_opens_empty_and_primary_key_uses_actual_selected_row() {
    let mut state = state();
    state.ribbon_tab = Tab::Create;
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::Button, "Table Design")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_design_visible());
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(editor.draft.is_design());
    assert!(editor.draft.fields().is_empty());
    assert!(!harness.state().0.can_save_table_draft());
    assert!(harness.state().1.is_empty());
    let name = add(
        &mut harness.state_mut().0,
        DraftFieldType::ShortText,
        "Name",
    );
    let key = add(&mut harness.state_mut().0, DraftFieldType::Number, "Key");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Key")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Primary Key")
        .click_accesskit();
    harness.run_steps(2);
    let fields = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .fields();
    assert_eq!(fields[0].id(), name);
    assert!(!fields[0].is_primary());
    assert_eq!(fields[1].id(), key);
    assert!(fields[1].is_primary());
    assert!(!fields[1].is_protected());
    assert!(
        harness
            .state_mut()
            .0
            .set_table_draft_field_type(key, DraftFieldType::LargeInteger)
    );
    assert!(
        !harness
            .state_mut()
            .0
            .set_table_draft_field_type(key, DraftFieldType::ShortText)
    );
    assert!(harness.state().1.is_empty());
    assert!(harness.state_mut().0.request_table_draft_field_delete(key));
    assert!(harness.state_mut().0.confirm_table_draft_field_delete());
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()
            .len(),
        1
    );
}

#[test]
fn keyless_prompt_yes_no_cancel_is_explicit_localized_and_preserves_original_fields() {
    for locale in Locale::ALL {
        for (theme, choice) in [
            (Theme::Light, "table-draft-property-no"),
            (Theme::Dark, "table-draft-property-yes"),
            (Theme::Light, "table-draft-cancel"),
        ] {
            let mut state = state();
            state.localizer = Localizer::new(locale).unwrap();
            assert!(state.open_table_design_draft());
            let original = add(&mut state, DraftFieldType::ShortText, "ID");
            state
                .table_draft
                .as_mut()
                .unwrap()
                .draft
                .set_field_description(original, Some("Retained 日本語"))
                .unwrap();
            let mut harness = harness(state);
            crate::theme::set_theme(&harness.ctx, theme);
            harness.run_steps(2);
            key_prompt(&mut harness);
            let label = harness.state().0.localizer.text(choice);
            harness
                .get_by_role_and_label(Role::Button, &label)
                .click_accesskit();
            harness.run_steps(2);
            if choice == "table-draft-cancel" {
                assert!(!harness.state().0.table_draft_save_open());
                assert!(harness.state().1.is_empty());
                let fields = harness
                    .state()
                    .0
                    .table_draft
                    .as_ref()
                    .unwrap()
                    .draft
                    .fields();
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].id(), original);
                assert_eq!(fields[0].description(), Some("Retained 日本語"));
                continue;
            }
            let [Action::SaveTableDraft { request }] = harness.state().1.as_slice() else {
                panic!("the choice issues exactly one save request");
            };
            let fields = request.snapshot.fields();
            if choice == "table-draft-property-no" {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].id(), original);
                assert!(!fields[0].is_primary());
            } else {
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name(), "ID1");
                assert!(fields[0].is_primary());
                assert!(fields[0].is_protected());
                assert_eq!(fields[1].id(), original);
                assert_eq!(fields[1].description(), Some("Retained 日本語"));
            }
            assert_eq!(
                harness.state().0.localizer.diagnostics().english_fallbacks,
                0
            );
        }
    }
}

#[test]
fn a_changed_declaration_invalidates_the_pending_keyless_decision() {
    let mut state = state();
    assert!(state.open_table_design_draft());
    let field = add(&mut state, DraftFieldType::Number, "Score");
    let mut harness = harness(state);
    key_prompt(&mut harness);
    harness
        .state_mut()
        .0
        .table_draft
        .as_mut()
        .unwrap()
        .draft
        .set_field_description(field, Some("Changed after prompt"))
        .unwrap();
    harness.run_steps(2);
    for label in ["Yes", "No"] {
        assert!(
            harness
                .get_by_role_and_label(Role::Button, label)
                .accesskit_node()
                .is_disabled()
        );
        harness.get_by_role_and_label(Role::Button, label).click();
        harness.run_steps(2);
        assert!(harness.state().1.is_empty());
    }
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().0.table_draft_save_open());
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .fields()[0]
            .description(),
        Some("Changed after prompt")
    );
}

#[test]
fn keyboard_key_decisions_keep_raw_record_text_and_default_to_yes() {
    for decision in ["yes", "no", "cancel"] {
        let mut state = state();
        assert!(state.open_table_design_draft());
        let field = add(&mut state, DraftFieldType::ShortText, "Name");
        let editor = state.table_draft.as_mut().unwrap();
        assert!(editor.record.synchronize(&editor.draft));
        assert!(editor.record.set_input(
            field,
            InsertInput::Value(EditInput {
                text: "  Ada 日本語  ".into(),
                is_null: false,
            })
        ));
        let mut harness = harness(state);
        key_prompt(&mut harness);
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Yes")
                .is_focused()
        );
        match decision {
            "yes" => harness.key_press(Key::Enter),
            "no" => {
                harness.key_press(Key::Tab);
                harness.run_steps(1);
                assert!(
                    harness
                        .get_by_role_and_label(Role::Button, "No")
                        .is_focused()
                );
                harness.key_press(Key::Enter);
            }
            _ => harness.key_press(Key::Escape),
        }
        harness.run_steps(2);
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        let snapshot = editor.draft.snapshot("Items").unwrap();
        let raw = editor.record.raw_texts(&snapshot).unwrap();
        let position = snapshot
            .fields()
            .iter()
            .position(|entry| entry.id() == field)
            .unwrap();
        assert_eq!(raw[position], "  Ada 日本語  ");
        assert_eq!(
            editor.record.inputs(&snapshot).unwrap()[position],
            InsertInput::Value(EditInput {
                text: "  Ada 日本語  ".into(),
                is_null: false,
            })
        );
        if decision == "cancel" {
            assert!(harness.state().1.is_empty());
            assert!(!harness.state().0.table_draft_save_open());
        } else {
            assert!(matches!(
                harness.state().1.as_slice(),
                [Action::SaveTableDraft { .. }]
            ));
            assert_eq!(
                snapshot.fields().len(),
                if decision == "yes" { 2 } else { 1 }
            );
            if decision == "yes" {
                assert_eq!(
                    editor.record.inputs(&snapshot).unwrap()[0],
                    InsertInput::Omitted
                );
            }
        }
    }
}

#[test]
fn keyless_no_is_a_single_decision_and_not_a_permanent_save_approval() {
    let mut state = state();
    assert!(state.open_table_design_draft());
    add(&mut state, DraftFieldType::Number, "Score");
    let mut harness = harness(state);
    key_prompt(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    let [Action::SaveTableDraft { request }] = harness.state().1.as_slice() else {
        panic!("one keyless request");
    };
    let id = request.id;
    assert!(
        harness
            .state_mut()
            .0
            .fail_table_draft_save(id, "Review refused".into())
    );
    harness.state_mut().1.clear();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_save_open());
    assert!(!harness.state().0.table_draft_save_pending());
    let title = harness.state().0.localizer.text("table-draft-no-key-title");
    assert!(harness.query_by_label(&title).is_some());
    assert!(harness.state().1.is_empty());
}

#[test]
fn datasheet_generated_id_keeps_primary_command_disabled_for_ordinary_fields_in_design() {
    let mut state = state();
    assert!(state.open_initial_table_draft());
    add(&mut state, DraftFieldType::Number, "Score");
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::Button, "Score")
        .click_accesskit();
    harness.run_steps(2);
    let before = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .snapshot("Items")
        .unwrap();
    assert!(!harness.state().0.can_toggle_table_draft_primary_key());
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Primary Key")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, "Primary Key")
        .click();
    harness.run_steps(2);
    assert!(!harness.state_mut().0.toggle_table_draft_primary_key());
    assert!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .snapshot("Items")
            .unwrap()
            == before
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn manual_primary_save_completion_preserves_first_supplied_record_into_native_draft() {
    let mut state = state();
    assert!(state.open_table_design_draft());
    let name = add(&mut state, DraftFieldType::ShortText, "Name");
    let key = add(&mut state, DraftFieldType::Number, "ActualKey");
    let editor = state.table_draft.as_mut().unwrap();
    editor.draft.set_primary_key(Some(key)).unwrap();
    assert!(editor.record.synchronize(&editor.draft));
    let expected = vec![
        InsertInput::Value(EditInput {
            text: "  Ada 日本語  ".into(),
            is_null: false,
        }),
        InsertInput::Value(EditInput {
            text: "0007".into(),
            is_null: false,
        }),
    ];
    assert!(editor.record.set_input(name, expected[0].clone()));
    assert!(editor.record.set_input(key, expected[1].clone()));
    let mut harness = harness(state);
    assert!(harness.state_mut().0.request_table_draft_save());
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(2);
    let mut actions = std::mem::take(&mut harness.state_mut().1);
    assert_eq!(actions.len(), 1);
    let Action::SaveTableDraft { request } = actions.remove(0) else {
        panic!("manual schema save");
    };
    assert!(harness.state_mut().0.begin_table_draft_save(&request));
    let table = request.snapshot.table_name().to_owned();
    let reopened = Database {
        document_revision: 72,
        title: "Guarded reopen".into(),
        title_path: None,
        format: "Access 2016".into(),
        tables: vec![crate::Table {
            local: true,
            name: table.clone(),
            system: false,
            storage: "Local".into(),
        }],
        queries: Vec::new(),
    };
    let state = &mut harness.state_mut().0;
    let completion = state
        .complete_table_draft_save(request.id, reopened)
        .expect("publish schema and preserve first supplied input");
    assert!(completion.record_pending);
    assert!(!state.has_unsaved_table_drafts());
    assert!(state.insert_continuation_pending());
    assert!(state.prepare_object_open(completion.target).is_some());
    assert!(state.set_table_data(crate::Data {
        source_revision: Some(72),
        row_ordinals: None,
        table: table.clone(),
        offset: 0,
        columns: vec!["Name".into(), "ActualKey".into()],
        rows: Vec::new(),
        has_more: false,
    }));
    let target = crate::InsertTarget {
        document_revision: 72,
        table,
    };
    assert!(state.can_insert_row(&target));
    state.begin_insert(target.clone(), 81);
    state.set_insert_draft(
        81,
        crate::InsertDraft {
            id: 91,
            target,
            fields: vec![
                crate::InsertField {
                    name: "Name".into(),
                    input_hint: crate::InputHint::Text,
                    nullable: true,
                    auto_number: false,
                },
                crate::InsertField {
                    name: "ActualKey".into(),
                    input_hint: crate::InputHint::Int32,
                    nullable: false,
                    auto_number: false,
                },
            ],
        },
    );
    assert!(!state.insert_continuation_pending());
    assert!(state.can_review_insert(91, &expected));
    assert!(state.data.as_ref().unwrap().rows.is_empty());
}

#[test]
fn primary_ribbon_selection_follows_actual_field_and_keeps_protected_key_readonly() {
    for theme in [Theme::Light, Theme::Dark] {
        let mut state = state();
        assert!(state.open_table_design_draft());
        add(&mut state, DraftFieldType::Number, "Key");
        let mut harness = harness(state);
        crate::theme::set_theme(&harness.ctx, theme);
        harness.run_steps(2);
        let toggled = |harness: &Harness<'_, (State, Vec<Action>)>| {
            harness
                .get_by_role_and_label(Role::Button, "Primary Key")
                .accesskit_node()
                .toggled()
        };
        assert_eq!(toggled(&harness), Some(Toggled::False));
        harness
            .get_by_role_and_label(Role::Button, "Primary Key")
            .click_accesskit();
        harness.run_steps(2);
        assert_eq!(toggled(&harness), Some(Toggled::True));
        assert!(harness.state().0.table_draft_primary_key_selected());
        harness
            .get_by_role_and_label(Role::Button, "Primary Key")
            .click_accesskit();
        harness.run_steps(2);
        assert_eq!(toggled(&harness), Some(Toggled::False));
        assert!(!harness.state().0.table_draft_primary_key_selected());
        assert!(harness.state().1.is_empty());
    }
    let mut state = state();
    assert!(state.open_initial_table_draft());
    assert!(state.show_table_draft_design());
    let harness = harness(state);
    let primary = harness.get_by_role_and_label(Role::Button, "Primary Key");
    assert!(primary.accesskit_node().is_disabled());
    assert_eq!(primary.accesskit_node().toggled(), Some(Toggled::True));
    assert!(harness.state().0.table_draft_primary_key_selected());
    assert!(!harness.state().0.can_toggle_table_draft_primary_key());
    assert!(harness.state().1.is_empty());
}

#[test]
fn deleting_the_last_manual_field_cannot_enable_a_nonfunctional_close_save() {
    let mut state = state();
    assert!(state.open_table_design_draft());
    let field = add(&mut state, DraftFieldType::Number, "OnlyField");
    assert!(state.request_table_draft_field_delete(field));
    assert!(state.confirm_table_draft_field_delete());
    let tab = state.active_object().unwrap();
    assert!(!state.close_object(&tab));
    assert!(!state.can_save_table_draft_close());
    let mut harness = harness(state);
    assert!(
        harness
            .get_all_by_role_and_label(Role::Button, "Save")
            .all(|save| save.accesskit_node().is_disabled())
    );
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run();
    assert!(harness.state().0.has_unsaved_table_drafts());
    assert!(!harness.state().0.table_draft_save_open());
    assert!(harness.state().1.is_empty());
}
