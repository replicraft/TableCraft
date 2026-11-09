//! App-private real header/save/witness interaction shared by Number acceptance cases.
use crate::workspace::{Desktop, test_support::finish_reads};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::time::{Duration, Instant};

pub(in crate::workspace) fn add_number(harness: &mut Harness<'_, Desktop>, column: &str) {
    let tab = harness.state().state.active_object().unwrap();
    assert!(harness.state().state.number_column_available);
    assert_eq!(
        harness
            .state()
            .state
            .number_column_inspection(&tab)
            .unwrap()
            .1,
        Some(true)
    );
    assert!(harness.state().state.can_name_number_column(&tab));
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Number")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(column);
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.number_column.is_some() {
        assert!(
            Instant::now() < deadline,
            "repeated Number save stalled: column={column}, report={:?}, error={:?}",
            harness.state().editing.save_report(),
            harness.state().state.error
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    finish_reads(harness);
}
