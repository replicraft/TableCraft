//! Real clipboard events enter the same source-bound review and save workflow.
use super::*;
#[cfg(any(target_os = "macos", target_os = "linux", windows))]
use crate::workspace::editing::test_support::{review_pasted_cell, save_reviewed_row};
use crate::workspace::test_support::{finish_reads, focus_cell, opened};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
#[test]
fn pasted_text_requires_review_and_explicit_apply_before_native_save() {
    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    review_pasted_cell(&mut harness, &path, &original, "Score", "2", "20", "25");
    save_reviewed_row(&mut harness, "Apply to original file");
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows[1][1].text, "25");
    assert_eq!(harness.state().state.selected_cell_text(), Some("25"));
    assert_ne!(std::fs::read(path).unwrap(), original);
}

#[test]
fn pasted_localized_numeric_text_is_not_silently_reinterpreted_or_saved() {
    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    harness.state_mut().state.formatting.system_locale = Some("de-DE".into());
    focus_cell(&mut harness, "Score", "2", "20");
    harness.event(egui::Event::Paste("1.234,56 €".into()));
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
            .value(),
        Some("1.234,56 €".into())
    );
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(harness.state().editing.review.is_none());
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn stale_paste_does_not_replace_the_native_request() {
    let (_directory, path, original, mut harness) = opened(mutation_fixture::database());
    focus_cell(&mut harness, "Score", "2", "20");
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    let target = EditTarget {
        document_revision: revision,
        table: "Items".into(),
        column: "Score".into(),
        parent_ordinal: 1,
    };
    let generation = harness.state().generation;
    let stale = EditTarget {
        document_revision: revision + 1,
        ..target.clone()
    };
    harness.state_mut().begin_paste_edit(stale, "25".into());
    assert_eq!(harness.state().generation, generation);
    assert!(!harness.state().state.edit_dialog_open());
    assert_eq!(std::fs::read(path).unwrap(), original);
}
