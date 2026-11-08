//! Leaving the inline row resolves its exact input before activating an old field.
use super::{
    field_label, harness,
    new_field::{choose_type, enter_name},
    state,
};
use crate::workspace::{EditInput, InsertInput};
use egui::{Event, Popup, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_builders::table::DraftFieldType;

#[test]
fn selecting_an_existing_row_commits_pending_field_once_and_restores_its_properties() {
    let (mut state, original) = state();
    assert!(state.table_draft.as_mut().unwrap().record.set_input(
        original,
        InsertInput::Value(EditInput {
            text: "  retained 日本語  ".into(),
            is_null: false,
        })
    ));
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    let before = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .revision();
    enter_name(&mut harness, "Score");
    choose_type(&mut harness, "design-type-number");
    harness
        .get_by_role_and_label(Role::Button, "Field1")
        .click();
    harness.run_steps(3);

    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields().len(), 3);
    assert_eq!(editor.draft.revision(), before + 1);
    assert_eq!(editor.draft.fields()[2].name(), "Score");
    assert_eq!(editor.draft.fields()[2].kind(), DraftFieldType::Number);
    assert_eq!(editor.design.selected, Some(original));
    assert!(!editor.new_field.pending());
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    assert_eq!(
        editor.record.inputs(&snapshot).unwrap()[1],
        InsertInput::Value(EditInput {
            text: "  retained 日本語  ".into(),
            is_null: false,
        })
    );
    assert!(matches!(
        editor.record.inputs(&snapshot).unwrap()[2],
        InsertInput::Omitted
    ));
    let size = harness.get_by_role_and_label(Role::TextInput, "Field Size");
    assert_eq!(size.value().as_deref(), Some("255"));
    assert!(!size.accesskit_node().is_disabled());
    harness.run_steps(3);
    assert_eq!(
        harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .revision(),
        before + 1
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn existing_name_opens_its_rename_only_after_the_new_field_is_committed() {
    let (mut state, original) = state();
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    enter_name(&mut harness, "Score");
    choose_type(&mut harness, "design-type-number");
    let label = field_label(&harness.state().0, "table-draft-design-name", "Field1");
    harness.get_by_role_and_label(Role::Button, &label).click();
    harness.run_steps(3);

    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields().len(), 3);
    assert_eq!(editor.draft.fields()[2].name(), "Score");
    assert_eq!(editor.draft.fields()[2].kind(), DraftFieldType::Number);
    let rename = editor
        .rename
        .as_ref()
        .expect("requested existing-field editor");
    assert_eq!(rename.field, original);
    assert_eq!(rename.name, "Field1");
    assert_eq!(editor.design.selected, Some(original));
    assert!(!editor.new_field.pending());
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field name")
            .is_focused()
    );
    assert!(harness.state().1.is_empty());
}

#[test]
fn invalid_pending_name_refuses_existing_selector_type_and_description_activation() {
    let (mut state, original) = state();
    let editor = state.table_draft.as_mut().unwrap();
    assert!(editor.record.set_input(
        original,
        InsertInput::Value(EditInput {
            text: "  retained 日本語  ".into(),
            is_null: false,
        })
    ));
    assert!(state.show_table_draft_design());
    let mut harness = harness(state);
    let before = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .snapshot("Table1")
        .unwrap();
    let inputs = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .record
        .inputs(&before)
        .unwrap();
    let record_revision = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .record
        .revision();
    enter_name(&mut harness, "Field1");
    choose_type(&mut harness, "design-type-number");
    assert!(
        !harness
            .state_mut()
            .0
            .set_table_draft_field_type(original, DraftFieldType::Currency)
    );

    let type_label = field_label(&harness.state().0, "table-draft-design-type", "Field1");
    let description_label = field_label(
        &harness.state().0,
        "table-draft-design-description",
        "Field1",
    );
    for (role, label) in [
        (Role::Button, "Field1".to_owned()),
        (Role::ComboBox, type_label),
        (Role::MultilineTextInput, description_label),
    ] {
        harness.get_by_role_and_label(role, &label).click();
        harness.run_steps(3);
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        assert!(editor.draft.snapshot("Table1").unwrap() == before);
        assert_eq!(editor.record.inputs(&before).unwrap(), inputs);
        assert_eq!(editor.record.revision(), record_revision);
        assert_eq!(editor.new_field.name, "Field1");
        assert_eq!(editor.new_field.kind, DraftFieldType::Number);
        assert!(editor.new_field.rejected);
        assert!(editor.rename.is_none());
        assert_eq!(editor.draft.fields()[1].description(), None);
        assert!(
            !Popup::is_any_open(&harness.ctx),
            "invalid activation must not open an old type popup"
        );
        let name = harness
            .state()
            .0
            .localizer
            .text("table-draft-design-new-name");
        assert!(
            harness
                .get_by_role_and_label(Role::TextInput, &name)
                .is_focused()
        );
    }
    assert!(harness.state().1.is_empty());
}

#[test]
fn same_frame_latest_name_text_and_existing_row_click_preserve_every_character() {
    for (initial, latest, kind) in [
        ("Scor", "e", DraftFieldType::Number),
        ("", "Score", DraftFieldType::ShortText),
    ] {
        let (mut state, original) = state();
        assert!(state.show_table_draft_design());
        let mut harness = harness(state);
        if !initial.is_empty() {
            enter_name(&mut harness, initial);
            choose_type(&mut harness, "design-type-number");
        }
        let label = harness
            .state()
            .0
            .localizer
            .text("table-draft-design-new-name");
        harness
            .get_by_role_and_label(Role::TextInput, &label)
            .focus();
        harness.run_steps(1);
        harness.event(Event::Text(latest.into()));
        harness
            .get_by_role_and_label(Role::Button, "Field1")
            .click();
        harness.run_steps(3);

        let editor = harness.state().0.table_draft.as_ref().unwrap();
        assert_eq!(editor.draft.fields().len(), 3, "initial={initial:?}");
        assert_eq!(editor.draft.fields()[2].name(), "Score");
        assert_eq!(editor.draft.fields()[2].kind(), kind);
        assert_eq!(editor.design.selected, Some(original));
        assert!(!editor.new_field.pending());
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn valid_pending_field_hands_off_to_actual_existing_type_and_description_controls() {
    for description in [false, true] {
        let (mut state, original) = state();
        let retained = InsertInput::Value(EditInput {
            text: "  retained 日本語  ".into(),
            is_null: false,
        });
        assert!(
            state
                .table_draft
                .as_mut()
                .unwrap()
                .record
                .set_input(original, retained.clone())
        );
        assert!(state.show_table_draft_design());
        let mut harness = harness(state);
        enter_name(&mut harness, "Score");
        choose_type(&mut harness, "design-type-number");
        let key = if description {
            "table-draft-design-description"
        } else {
            "table-draft-design-type"
        };
        let label = field_label(&harness.state().0, key, "Field1");
        let role = if description {
            Role::MultilineTextInput
        } else {
            Role::ComboBox
        };
        harness.get_by_role_and_label(role, &label).click();
        harness.run_steps(4);
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        assert_eq!(editor.draft.fields().len(), 3);
        assert_eq!(editor.draft.fields()[2].name(), "Score");
        assert_eq!(editor.draft.fields()[2].kind(), DraftFieldType::Number);
        assert_eq!(editor.design.selected, Some(original));
        assert!(!editor.new_field.pending());

        if description {
            assert!(harness.get_by_role_and_label(role, &label).is_focused());
            harness
                .get_by_role_and_label(role, &label)
                .type_text("  Description 日本語  ");
            harness.run_steps(3);
        } else {
            assert!(
                Popup::is_any_open(&harness.ctx),
                "actual existing type popup must open after the pending commit"
            );
            harness.get_by_role_and_label(role, &label).click();
            harness.run_steps(3);
            assert!(
                !Popup::is_any_open(&harness.ctx),
                "closing a live type popup must not queue its reopening"
            );
            harness.get_by_role_and_label(role, &label).click();
            harness.run_steps(2);
            let currency = harness.state().0.localizer.text("design-type-currency");
            harness
                .get_by_role_and_label(Role::Button, &currency)
                .click_accesskit();
            harness.run_steps(3);
        }
        let editor = harness.state().0.table_draft.as_ref().unwrap();
        assert_eq!(editor.draft.fields()[1].id(), original);
        assert_eq!(
            editor.draft.fields()[1].kind(),
            if description {
                DraftFieldType::ShortText
            } else {
                DraftFieldType::Currency
            }
        );
        assert_eq!(
            editor.draft.fields()[1].description(),
            description.then_some("  Description 日本語  ")
        );
        assert_eq!(editor.draft.fields()[2].name(), "Score");
        assert_eq!(editor.draft.fields()[2].kind(), DraftFieldType::Number);
        assert_eq!(editor.draft.fields()[2].description(), None);
        let snapshot = editor.draft.snapshot("Table1").unwrap();
        assert_eq!(editor.record.inputs(&snapshot).unwrap()[1], retained);
        assert!(harness.state().1.is_empty());
    }
}
