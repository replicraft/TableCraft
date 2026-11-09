//! Source-free keyboard/disabled-command checks, separate from host qualification.
use super::{confirming, design, harness, observed};
use crate::{Action, FieldRenameTarget};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;

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
    assert!(state.set_field_rename_inspection_by_id(
        81,
        7,
        "Table1",
        "Score",
        17,
        true,
        Some(FieldRenameNameProfile::SameWidthAscii { units: 5 })
    ));
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

#[test]
fn confirmation_retry_is_only_for_terminal_inspection_and_keeps_the_exact_target() {
    for eligible in [false, true] {
        let (mut state, _) = if eligible { observed() } else { confirming() };
        if !eligible {
            let target = state.field_drop_inspection_target().unwrap();
            state
                .prepare_field_drop_inspection(&target)
                .unwrap()
                .bind(80);
        }
        let label = state.localizer.text("field-drop-retry-inspection");
        let harness = harness(state);
        assert!(
            harness
                .query_by_role_and_label(Role::Button, &label)
                .is_none()
        );
        assert_eq!(
            harness
                .get_by_role_and_label(Role::Button, "Yes")
                .accesskit_node()
                .is_disabled(),
            !eligible
        );
        harness.get_by_role_and_label(Role::Button, "No");
        assert!(harness.state().1.is_empty());
    }

    for terminal in 0..3 {
        let (mut state, target) = confirming();
        state
            .prepare_field_drop_inspection(&target)
            .unwrap()
            .bind(80);
        match terminal {
            0 => assert!(state.set_field_drop_source(&target, 80, false)),
            1 => assert!(state.interrupt_field_drop_inspection_id(80)),
            _ => assert!(state.fail_field_drop_inspection_by_id(80, "Inspection failed".into())),
        }
        let label = state.localizer.text("field-drop-retry-inspection");
        let mut harness = harness(state);
        assert!(
            harness
                .get_by_role_and_label(Role::Button, "Yes")
                .accesskit_node()
                .is_disabled()
        );
        harness
            .get_by_role_and_label(Role::Button, &label)
            .click_accesskit();
        // Retry changes the modal after this frame's readonly collector ran.
        // The next frame emits and the harness binds the exact inspection.
        harness.run_steps(2);
        let [Action::InspectFieldDrop { target: outgoing }] = harness.state().1.as_slice() else {
            panic!("retry emits only the exact readonly inspection");
        };
        assert_eq!(outgoing, &target);
        assert!(!harness.state().0.field_drop_dirty());
        assert_eq!(
            harness.state().0.data.as_ref().unwrap().columns[1].name,
            "Score"
        );
        assert!(
            harness
                .query_by_role_and_label(Role::Button, &label)
                .is_none()
        );
    }
}
