//! Actual Design commands reuse the exact revision-bound field confirmation.
use super::tests::{harness, ids, manual_primary_harness, set_input};
use crate::{FluentArgs, workspace::State};
use egui::{Event, Key, Modifiers, PointerButton, Popup, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};

fn field_label(state: &State, key: &str, field: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("field", field);
    state.localizer.format(key, &args)
}

#[test]
fn design_ribbon_delete_manual_primary_cancel_then_confirm_preserves_other_raw_input() {
    let (mut harness, key) = manual_primary_harness();
    let fields = ids(&harness);
    set_input(
        &mut harness.state_mut().0,
        fields[0],
        "  Ada 日本語  ",
        false,
    );
    assert!(harness.state_mut().0.show_table_draft_design());
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "ManualKey")
        .click();
    harness.run_steps(3);
    let before = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .draft
        .snapshot("Table1")
        .unwrap();
    let raw = harness
        .state()
        .0
        .table_draft
        .as_ref()
        .unwrap()
        .record
        .raw_texts(&before)
        .unwrap();
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Delete Rows")
            .accesskit_node()
            .is_disabled()
    );
    harness
        .get_by_role_and_label(Role::Button, "Delete Rows")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.table_draft_field_delete_open());
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(editor.draft.snapshot("Table1").unwrap() == before);
    assert_eq!(editor.record.raw_texts(&before).unwrap(), raw);
    assert!(
        editor
            .draft
            .fields()
            .iter()
            .any(|field| field.id() == key && field.is_primary())
    );

    harness
        .get_by_role_and_label(Role::Button, "Delete Rows")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Delete field")
        .click_accesskit();
    harness.run_steps(3);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(editor.draft.fields().len(), 1);
    assert_eq!(editor.draft.fields()[0].id(), fields[0]);
    assert!(!editor.draft.fields()[0].is_primary());
    let after = editor.draft.snapshot("Table1").unwrap();
    assert_eq!(editor.record.raw_texts(&after).unwrap(), ["  Ada 日本語  "]);
    assert!(!harness.state().0.table_draft_field_delete_open());
    assert!(harness.state().1.is_empty());
}

#[test]
fn generated_id_is_disabled_but_delete_on_focused_existing_selector_requests_confirmation() {
    let mut harness = harness();
    let fields = ids(&harness);
    assert!(harness.state_mut().0.show_table_draft_design());
    harness.run_steps(3);
    harness.get_by_role_and_label(Role::Button, "ID").click();
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Delete Rows")
            .accesskit_node()
            .is_disabled()
    );
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    assert!(!harness.state().0.table_draft_field_delete_open());
    assert_eq!(ids(&harness), fields);

    harness
        .get_by_role_and_label(Role::Button, "Field1")
        .click();
    harness.run_steps(3);
    assert!(
        harness
            .get_by_role_and_label(Role::Button, "Field1")
            .is_focused()
    );
    for modifiers in [
        Modifiers::SHIFT,
        Modifiers::ALT,
        Modifiers::CTRL,
        Modifiers::COMMAND,
    ] {
        harness.key_press_modifiers(modifiers, Key::Delete);
        harness.run_steps(2);
        assert!(
            !harness.state().0.table_draft_field_delete_open(),
            "modified Delete must not remove a field: {modifiers:?}"
        );
        assert_eq!(ids(&harness), fields);
    }
    harness.key_press(Key::Delete);
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_field_delete_open());
    assert_eq!(ids(&harness), fields);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(ids(&harness), fields);
    assert!(harness.state().1.is_empty());
}

#[test]
fn delete_in_name_description_blank_row_or_open_type_popup_never_requests_field_deletion() {
    for context in ["name", "description", "new field", "popup"] {
        let mut harness = harness();
        let fields = ids(&harness);
        assert!(harness.state_mut().0.show_table_draft_design());
        harness.run_steps(3);
        harness
            .get_by_role_and_label(Role::Button, "Field1")
            .click();
        harness.run_steps(2);
        match context {
            "name" => {
                let label = field_label(&harness.state().0, "table-draft-design-name", "Field1");
                harness.get_by_role_and_label(Role::Button, &label).click();
                harness.run_steps(2);
                harness
                    .get_by_role_and_label(Role::TextInput, "Field name")
                    .focus();
            }
            "description" => {
                let label = field_label(
                    &harness.state().0,
                    "table-draft-design-description",
                    "Field1",
                );
                harness
                    .get_by_role_and_label(Role::MultilineTextInput, &label)
                    .focus();
            }
            "new field" => {
                let label = harness
                    .state()
                    .0
                    .localizer
                    .text("table-draft-design-new-name");
                harness
                    .get_by_role_and_label(Role::TextInput, &label)
                    .focus();
            }
            _ => {
                let label = field_label(&harness.state().0, "table-draft-design-type", "Field1");
                harness
                    .get_by_role_and_label(Role::ComboBox, &label)
                    .click();
            }
        }
        harness.run_steps(2);
        if context == "popup" {
            assert!(Popup::is_any_open(&harness.ctx));
        }
        harness.key_press(Key::Delete);
        harness.run_steps(2);
        assert!(
            !harness.state().0.table_draft_field_delete_open(),
            "context={context}"
        );
        assert_eq!(ids(&harness), fields, "context={context}");
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn actual_ribbon_confirmation_refuses_a_stale_raw_property_clock() {
    let mut harness = harness();
    let fields = ids(&harness);
    set_input(&mut harness.state_mut().0, fields[2], "0007", false);
    assert!(harness.state_mut().0.show_table_draft_design());
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Field1")
        .click();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Delete Rows")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().0.table_draft_field_delete_open());
    let editor = harness.state_mut().0.table_draft.as_mut().unwrap();
    let before = editor.draft.snapshot("Table1").unwrap();
    let inputs = editor.record.inputs(&before).unwrap();
    // Simulate a late raw-editor update against the captured confirmation,
    // without changing schema or record revisions. The raw clock owns refusal.
    assert!(editor.properties.record_raw_edit());
    harness
        .get_by_role_and_label(Role::Button, "Delete field")
        .click_accesskit();
    harness.run_steps(3);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(editor.draft.snapshot("Table1").unwrap() == before);
    assert_eq!(editor.record.inputs(&before).unwrap(), inputs);
    assert_eq!(ids(&harness), fields);
    assert!(!harness.state().0.table_draft_field_delete_open());
    assert!(harness.state().1.is_empty());
}

#[test]
fn temporary_design_ribbon_delete_cancel_closes_the_strip_in_normal_and_synthetic_hosts() {
    for synthetic in [false, true] {
        let mut harness = harness();
        harness.state_mut().0.mock_preview = synthetic;
        let fields = ids(&harness);
        assert!(harness.state_mut().0.show_table_draft_design());
        harness.run_steps(3);
        harness
            .get_by_role_and_label(Role::Button, "Field1")
            .click();
        harness.run_steps(2);
        harness.state_mut().0.set_ribbon_collapsed(true);
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Tab, "Table Design")
            .click_accesskit();
        harness.run_steps(3);
        assert!(harness.state().0.ribbon_temporarily_open());
        let delete = harness.get_by_role_and_label(Role::Button, "Delete Rows");
        assert!(!delete.accesskit_node().is_disabled());
        delete.click_accesskit();
        harness.run_steps(3);
        assert!(harness.state().0.table_draft_field_delete_open());
        assert!(!harness.state().0.ribbon_temporarily_open());
        harness
            .get_by_role_and_label(Role::Button, "Cancel")
            .click_accesskit();
        harness.run_steps(3);
        assert!(!harness.state().0.table_draft_field_delete_open());
        assert!(harness.state().0.ribbon_collapsed());
        assert!(!harness.state().0.ribbon_temporarily_open());
        assert!(
            harness
                .query_by_role_and_label(Role::Button, "Delete Rows")
                .is_none()
        );
        assert_eq!(ids(&harness), fields);
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn same_batch_focus_transfer_or_pointer_press_with_delete_cannot_use_the_old_selector() {
    for target in ["description focus", "new field focus", "description press"] {
        let mut harness = harness();
        let fields = ids(&harness);
        assert!(harness.state_mut().0.show_table_draft_design());
        harness.run_steps(3);
        harness
            .get_by_role_and_label(Role::Button, "Field1")
            .click();
        harness.run_steps(2);
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Field1")
                .is_focused()
        );
        let before = harness
            .state()
            .0
            .table_draft
            .as_ref()
            .unwrap()
            .draft
            .snapshot("Table1")
            .unwrap();
        let description = field_label(
            &harness.state().0,
            "table-draft-design-description",
            "Field2",
        );
        let mut pressed_at = None;
        if target == "new field focus" {
            let name = harness
                .state()
                .0
                .localizer
                .text("table-draft-design-new-name");
            harness
                .get_by_role_and_label(Role::TextInput, &name)
                .focus();
        } else if target == "description focus" {
            harness
                .get_by_role_and_label(Role::MultilineTextInput, &description)
                .focus();
        } else {
            let position = harness
                .get_by_role_and_label(Role::MultilineTextInput, &description)
                .rect()
                .center();
            harness.event(Event::PointerMoved(position));
            harness.event(Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            });
            pressed_at = Some(position);
        }
        // No intervening UI pass: the genuine focus/press and Delete belong to
        // one event batch while the old selector was previously focused.
        harness.key_press(Key::Delete);
        harness.run_steps(2);
        assert!(
            !harness.state().0.table_draft_field_delete_open(),
            "target={target}"
        );
        assert_eq!(ids(&harness), fields);
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
                == before
        );
        if let Some(position) = pressed_at {
            harness.event(Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            });
            harness.run_steps(1);
        }
        assert!(harness.state().1.is_empty());
    }
}

#[test]
fn same_frame_description_text_and_ribbon_delete_capture_the_updated_revision() {
    let mut harness = harness();
    let fields = ids(&harness);
    set_input(&mut harness.state_mut().0, fields[2], "0007", false);
    assert!(harness.state_mut().0.show_table_draft_design());
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Field1")
        .click();
    harness.run_steps(2);
    let label = field_label(
        &harness.state().0,
        "table-draft-design-description",
        "Field1",
    );
    harness
        .get_by_role_and_label(Role::MultilineTextInput, &label)
        .focus();
    harness.run_steps(2);
    harness.event(Event::Text("  latest 日本語  ".into()));
    harness
        .get_by_role_and_label(Role::Button, "Delete Rows")
        .click_accesskit();
    harness.run_steps(3);
    assert!(harness.state().0.table_draft_field_delete_open());
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert_eq!(
        editor.draft.fields()[1].description(),
        Some("  latest 日本語  ")
    );
    let confirmation = harness.state().0.table_draft_field_delete.as_ref().unwrap();
    assert_eq!(confirmation.schema_revision, editor.draft.revision());
    assert_eq!(confirmation.record_revision, editor.record_revision());
    assert_eq!(confirmation.property_revision, editor.property_revision());
    harness
        .get_by_role_and_label(Role::Button, "Delete field")
        .click_accesskit();
    harness.run_steps(3);
    let editor = harness.state().0.table_draft.as_ref().unwrap();
    assert!(
        !editor
            .draft
            .fields()
            .iter()
            .any(|field| field.id() == fields[1])
    );
    let snapshot = editor.draft.snapshot("Table1").unwrap();
    assert_eq!(editor.record.raw_texts(&snapshot).unwrap(), ["", "0007"]);
    assert!(harness.state().1.is_empty());
}
