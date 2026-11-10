//! Actual field-add, edit/review/save and guarded reopen over historical rows.
use crate::workspace::{
    Desktop,
    test_support::{app_harness, field_addition::add_field, finish_reads, finish_save, opened},
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::path::Path;
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::{SaveOutcome, journal_path};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated;
use tablecraft_ui::{CellKind, EditTarget};

pub(super) fn begin_replacement(
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
    harness.state_mut().begin_edit(EditTarget {
        document_revision: revision,
        table: "Table1".into(),
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

pub(super) fn review_and_save(
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
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(!journal_path(path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Apply to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("Historical existing-field edit did not save: {report:?}");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    assert_eq!(receipt.matched_rows, Some(1));
    assert_eq!(std::fs::read(path).unwrap(), expected);
    assert!(!journal_path(path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.edit_dialog_open());
}

fn assert_bonus_nulls(harness: &Harness<'_, Desktop>) {
    for row in &harness.state().state.data.as_ref().unwrap().rows {
        assert_eq!(row[2].text, "NULL");
        assert!(matches!(
            row[2].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    }
}

#[test]
fn added_number_keeps_historical_nulls_while_existing_score_is_edited_saved_and_reopened() {
    let [before, after_add] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let (_directory, path, _, mut harness) = opened(before);
    add_field(&mut harness, "Bonus", FieldAdditionKind::Number);
    assert_eq!(std::fs::read(&path).unwrap(), after_add);

    let supplied = populated::update_scores(&after_add, &[(0, Some(8))]);
    begin_replacement(&mut harness, 0, "Score", false, Some("8"));
    review_and_save(&mut harness, &path, &after_add, &supplied);
    assert_bonus_nulls(&harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][1].text,
        "8"
    );

    let from_null = populated::update_scores(&supplied, &[(1, Some(12))]);
    begin_replacement(&mut harness, 1, "Score", true, Some("12"));
    review_and_save(&mut harness, &path, &supplied, &from_null);
    assert_bonus_nulls(&harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[1][1].text,
        "12"
    );

    let to_null = populated::update_scores(&from_null, &[(2, None)]);
    begin_replacement(&mut harness, 2, "Score", false, None);
    review_and_save(&mut harness, &path, &from_null, &to_null);
    assert_bonus_nulls(&harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[2][1].text,
        "NULL"
    );
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let page = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), 3);
    for (position, score) in ["8", "12", "NULL"].into_iter().enumerate() {
        assert_eq!(page.rows[position][1].text, score);
        assert_eq!(page.rows[position][2].text, "NULL");
        assert!(matches!(
            page.rows[position][2].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    }
    assert_eq!(std::fs::read(&path).unwrap(), to_null);
    assert!(!journal_path(&path).exists());
}

#[test]
fn absent_added_number_null_noop_is_not_materialized_by_the_normal_review_path() {
    let [before, after_add] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let (_directory, path, _, mut harness) = opened(before);
    add_field(&mut harness, "Bonus", FieldAdditionKind::Number);
    begin_replacement(&mut harness, 0, "Bonus", true, None);
    let generation = harness.state().generation;
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    assert_ne!(harness.state().generation, generation);
    // Refusal is an expected runtime response; finish_reads intentionally asserts
    // successful operations, so consume this response without that helper.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(
            std::time::Instant::now() < deadline,
            "Absent-field review stalled"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(
        harness
            .query_by_role_and_label(Role::Button, "Apply to original file")
            .is_none()
    );
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), after_add);
    assert!(!journal_path(&path).exists());
}
