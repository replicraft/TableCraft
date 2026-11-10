//! Ordinary clipboard/review/save widgets shared by real-file host journeys.
use crate::workspace::Desktop;
use crate::workspace::test_support::{finish_reads, finish_save, focus_cell};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::path::Path;
use tablecraft_runtime::{SaveOutcome, journal_path};

pub(in crate::workspace) fn review_pasted_cell(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    column: &str,
    row: &str,
    original: &str,
    replacement: &str,
) {
    focus_cell(harness, column, row, original);
    harness.event(egui::Event::Paste(replacement.into()));
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
            .value()
            .as_deref(),
        Some(replacement)
    );
    assert!(harness.state().editing.review.is_none());
    assert_eq!(std::fs::read(path).unwrap(), before);
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().editing.review.is_some());
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(!journal_path(path).exists());
}

pub(in crate::workspace) fn review_row_delete(harness: &mut Harness<'_, Desktop>, row: usize) {
    let ordinal = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .parent_ordinal(row)
        .unwrap();
    harness
        .get_all_by_role(Role::RowHeader)
        .nth(row)
        .unwrap()
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .state
            .current_delete_target()
            .unwrap()
            .parent_ordinal,
        ordinal
    );
    harness
        .get_by_role_and_label(Role::Button, "Delete")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Delete row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(harness.state().editing.delete_review.is_some());
}

pub(in crate::workspace) fn save_reviewed_row(harness: &mut Harness<'_, Desktop>, button: &str) {
    harness
        .get_by_role_and_label(Role::Button, button)
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1) && receipt.matched_rows == Some(1)),
        "{report:?}"
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
}
