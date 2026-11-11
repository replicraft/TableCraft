//! Actual cell-edit widgets shared by app-private schema acceptance journeys.
use super::{finish_reads, finish_save};
use crate::workspace::Desktop;
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::path::Path;
use tablecraft_runtime::{SaveOutcome, journal_path};
use tablecraft_ui::EditTarget;

pub(in crate::workspace) fn begin_replacement(
    harness: &mut Harness<'_, Desktop>,
    row: u32,
    column: &str,
    original_null: bool,
    value: Option<&str>,
) {
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    let table = harness.state().state.data.as_ref().unwrap().table.clone();
    harness.state_mut().begin_edit(EditTarget {
        document_revision: revision,
        table,
        column: column.into(),
        parent_ordinal: row,
    });
    finish_reads(harness);
    assert!(harness.state().state.edit_dialog_open());
    if original_null != value.is_none() {
        harness
            .get_by_role_and_label(Role::CheckBox, "Use NULL (subject to validation)")
            .click_accesskit();
        harness.run_steps(1);
    }
    if let Some(value) = value {
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
            .focus();
        harness.run_steps(1);
        harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
        if value.is_empty() {
            // An empty text event is ignored by egui; delete the selection just
            // as the user does when explicitly supplying a zero-length value.
            harness.key_press(Key::Backspace);
        } else {
            harness
                .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
                .type_text(value);
        }
        harness.run_steps(2);
    }
}

pub(in crate::workspace) fn review_and_save(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    expected: &[u8],
) {
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert_image(path, before);
    assert!(!journal_path(path).exists());
    let apply = harness.query_by_role_and_label(Role::Button, "Apply to original file");
    let Some(apply) = apply else {
        let messages = harness
            .query_all_by_role(Role::Label)
            .filter_map(|node| node.value())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(8)
            .map(|message| message.chars().take(200).collect::<String>())
            .collect::<Vec<_>>();
        panic!("cell review did not authorize Apply; final visible messages: {messages:?}");
    };
    apply.click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("Historical existing-field edit did not save: {report:?}");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    assert_eq!(receipt.matched_rows, Some(1));
    assert_image(path, expected);
    assert!(!journal_path(path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.edit_dialog_open());
}

fn assert_image(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let difference = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && difference.is_none(),
        "complete cell-save image lengths {} / {}, first mismatch {:?}",
        actual.len(),
        expected.len(),
        difference
    );
}
