//! Actual field-name proposals and consuming saves shared across CRUD journeys.
use super::{finish_reads, finish_schema_save};
use crate::workspace::Desktop;
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use tablecraft_runtime::journal_path;

pub(in crate::workspace) fn propose_name(
    harness: &mut Harness<'_, Desktop>,
    original_name: &str,
    replacement: &str,
) {
    harness.get_by_label(original_name).click();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run_steps(2);
    let command = harness.get_by_role_and_label(Role::Button, "Rename Field");
    assert!(!command.accesskit_node().is_disabled());
    command.click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    let input = harness.get_by_role_and_label(Role::TextInput, "Field name");
    assert!(
        !input.accesskit_node().is_read_only()
            && !input.accesskit_node().is_disabled()
            && harness.state().state.can_edit_field_rename(),
        "actual native source inspection must permit the {original_name:?} proposal"
    );
    input.focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(replacement);
    harness.run_steps(1);
}

pub(in crate::workspace) fn rename_and_save(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    after: &[u8],
    old: &str,
    new: &str,
) {
    propose_name(harness, old, new);
    assert_eq!(std::fs::read(path).unwrap(), before);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    if old == new {
        let deadline = Instant::now() + Duration::from_secs(10);
        while harness.state().state.busy {
            assert!(Instant::now() < deadline, "Unchanged rename review stalled");
            assert!(!harness.state().editing.has_save());
            harness.run_steps(1);
            std::thread::yield_now();
        }
        finish_reads(harness);
    } else {
        finish_schema_save(harness);
    }
    assert_eq!(std::fs::read(path).unwrap(), after);
    assert!(!journal_path(path).exists());
    assert!(!harness.state().state.field_rename_open());
}
