//! Actual header editor actions retain the stored name until native publication.
use super::test_support::{harness, inspecting, observed};
use crate::Action;
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::Queryable;
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;

#[test]
fn ctrl_s_captures_post_widget_true_name_and_actual_native_number() {
    let (state, target) = observed();
    let mut harness = harness(state);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text("Total");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let [Action::ReviewFieldRename { request }] = harness.state().1.as_slice() else {
        panic!("exactly one field rename review expected");
    };
    assert_eq!(request.target, target);
    assert_eq!(request.target.native_number, 17);
    assert_eq!(request.new_name, "Total");
    assert_eq!(
        harness.state().0.data.as_ref().unwrap().columns[1].name,
        "Score"
    );
}

#[test]
fn unsupported_width_stays_visible_and_never_dispatches_a_review() {
    let (mut state, target) = observed();
    assert!(state.set_field_rename_name(&target, "Longer".into()));
    let mut harness = harness(state);
    harness.get_by_role_and_label(Role::TextInput, "Field name");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert_eq!(
        harness.state().0.field_rename_proposal().unwrap().1,
        "Longer"
    );
    assert_eq!(
        harness.state().0.data.as_ref().unwrap().columns[1].name,
        "Score"
    );
}

#[test]
fn fitted_observation_allows_growing_and_shrinking_only_as_exact_review_requests() {
    for proposed in ["LongerScore", "Pts"] {
        let (mut state, target) = inspecting();
        assert!(state.set_field_rename_inspection_by_id(
            80,
            7,
            "Table1",
            "Score",
            17,
            true,
            Some(FieldRenameNameProfile::FittedAscii { original_units: 5 }),
        ));
        let mut harness = harness(state);
        harness
            .get_by_role_and_label(Role::TextInput, "Field name")
            .focus();
        harness.run_steps(1);
        harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
        harness
            .get_by_role_and_label(Role::TextInput, "Field name")
            .type_text(proposed);
        harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
        harness.run_steps(1);

        let [Action::ReviewFieldRename { request }] = harness.state().1.as_slice() else {
            panic!("one exact fitted name review, never a save, must be dispatched");
        };
        assert_eq!(request.target, target);
        assert_eq!(request.new_name, proposed);
        assert_eq!(
            harness.state().0.data.as_ref().unwrap().columns[1].name,
            "Score"
        );
    }
}

#[test]
fn fitted_policy_still_refuses_invalid_observations_and_non_ascii_shapes() {
    let (mut state, target) = inspecting();
    for original_units in [0, 4, 65] {
        assert!(!state.set_field_rename_inspection_by_id(
            80,
            7,
            "Table1",
            "Score",
            17,
            true,
            Some(FieldRenameNameProfile::FittedAscii { original_units }),
        ));
    }
    assert!(state.set_field_rename_inspection_by_id(
        80,
        7,
        "Table1",
        "Score",
        17,
        true,
        Some(FieldRenameNameProfile::FittedAscii { original_units: 5 }),
    ));
    for proposed in ["Scoré", ""] {
        assert!(state.set_field_rename_name(&target, proposed.into()));
        assert!(!state.can_review_field_rename());
        assert_eq!(state.field_rename_proposal().unwrap().1, proposed);
    }
    let mut harness = harness(state);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    assert!(harness.state().1.is_empty());
    assert_eq!(
        harness.state().0.data.as_ref().unwrap().columns[1].name,
        "Score"
    );
}
