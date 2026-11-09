//! Source-free keyboard/disabled-command checks, separate from host qualification.
use super::{design, harness};
use crate::FieldRenameTarget;
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn unavailable_design_delete_rows_stays_visible_and_keyboard_cannot_dispatch() {
    let mut state = design();
    state.field_drop_available = false;
    let mut harness = harness(state);
    let delete = harness.get_by_role_and_label(Role::Button, "Delete Rows");
    assert!(delete.accesskit_node().is_disabled());
    delete.click();
    harness.run_steps(1);
    harness.get_by_label("Score").focus();
    harness.run_steps(1);
    harness.key_press(Key::Delete);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert!(!harness.state().0.field_drop_open());
    assert_eq!(
        harness.state().0.schema.as_ref().unwrap().fields[0].number,
        17
    );
}

#[test]
fn delete_in_saved_design_name_input_edits_proposal_without_deleting_the_field() {
    let mut state = design();
    state.field_rename_available = true;
    let target = FieldRenameTarget {
        tab: state.active_object().unwrap(),
        column: "Score".into(),
        native_number: 17,
    };
    assert!(state.begin_field_rename(target));
    let outgoing = state.field_rename_inspection_target().unwrap();
    state
        .prepare_field_rename_inspection(&outgoing)
        .unwrap()
        .bind(81);
    assert!(state.set_field_rename_inspection_by_id(81, 7, "Table1", "Score", 17, true, Some(5)));
    let mut harness = harness(state);
    let label = harness.state().0.localizer.text("field-rename-name");
    harness
        .get_by_role_and_label(Role::TextInput, &label)
        .focus();
    harness.run_steps(1);
    assert!(
        harness
            .get_by_role_and_label(Role::TextInput, &label)
            .is_focused()
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.key_press(Key::Delete);
    harness.run_steps(1);
    assert_eq!(harness.state().0.field_rename_proposal().unwrap().1, "");
    assert!(harness.state().1.is_empty());
    assert!(!harness.state().0.field_drop_open());
    let field = &harness.state().0.schema.as_ref().unwrap().fields[0];
    assert_eq!(field.name, "Score");
    assert_eq!(field.number, 17);
}
