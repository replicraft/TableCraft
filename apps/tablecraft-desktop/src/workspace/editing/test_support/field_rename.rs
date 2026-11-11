//! Actual field-name proposals and consuming saves shared across CRUD journeys.
use crate::workspace::Desktop;
use crate::workspace::test_support::{finish_reads, finish_schema_save};
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
use tablecraft_ui::FieldRenameSaveIdentity;

pub(in crate::workspace::editing) fn propose_name(
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

pub(in crate::workspace::editing) fn rename_and_save(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    after: &[u8],
    old: &str,
    new: &str,
) {
    propose_name(harness, old, new);
    assert_image(path, before);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    if old == new {
        // A real unchanged review may retain its read-witness owner. Only the
        // native save actor must remain absent, even if review finished quickly.
        no_native_actor(harness);
        let deadline = Instant::now() + Duration::from_secs(10);
        while harness.state().state.busy || harness.state().editing.field_rename.is_some() {
            assert!(Instant::now() < deadline, "Unchanged rename review stalled");
            no_native_actor(harness);
            harness.run_steps(1);
            std::thread::yield_now();
        }
        finish_reads(harness);
        no_native_actor(harness);
        assert!(!harness.state().editing.has_save());
    } else {
        finish_schema_save(harness);
    }
    assert_image(path, after);
    assert!(!journal_path(path).exists());
    assert!(!harness.state().state.field_rename_open());
}

fn no_native_actor(harness: &Harness<'_, Desktop>) {
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    if let Some(host) = &harness.state().editing.field_rename {
        assert!(matches!(
            host.identity,
            None | Some(FieldRenameSaveIdentity::UnchangedReview { .. })
        ));
    }
}

fn assert_image(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let difference = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && difference.is_none(),
        "complete field-rename image lengths {} / {}, first mismatch {:?}",
        actual.len(),
        expected.len(),
        difference
    );
}
