//! App-private real header/save/witness interaction shared by field-addition acceptance cases.
use crate::workspace::{Desktop, test_support::finish_reads};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::time::{Duration, Instant};
use tablecraft_builders::table::FieldAdditionKind;

pub(in crate::workspace) fn add_field(
    harness: &mut Harness<'_, Desktop>,
    column: &str,
    kind: FieldAdditionKind,
) {
    let tab = harness.state().state.active_object().unwrap();
    assert!(harness.state().state.field_addition_available);
    assert_eq!(
        harness
            .state()
            .state
            .field_addition_inspection(&tab)
            .unwrap()
            .1,
        Some(true)
    );
    assert!(harness.state().state.can_name_field_addition(&tab, kind));
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    let key = match kind {
        FieldAdditionKind::Number => "table-draft-number",
        FieldAdditionKind::ShortText => "table-draft-short-text",
    };
    let label = harness.state().state.localizer.text(key);
    harness
        .get_by_role_and_label(Role::Button, &label)
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
    while harness.state().state.busy || harness.state().editing.has_save() {
        assert!(
            Instant::now() < deadline,
            "field save stalled: column={column}, report={:?}, error={:?}",
            harness.state().editing.save_report(),
            harness.state().state.error
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    finish_reads(harness);
}
