//! Actual insertion widgets shared by app-private schema and record acceptance.
use super::{finish_reads, finish_save};
use crate::workspace::Desktop;
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::path::Path;
use tablecraft_runtime::{SaveOutcome, journal_path};
use tablecraft_ui::CellKind;

pub(in crate::workspace) fn review_insert(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

/// Open the actual generated-ID row editor without supplying an ID value.
pub(in crate::workspace) fn begin_generated_insert(harness: &mut Harness<'_, Desktop>) {
    // The ordinary Ribbon starts a real source-bound insertion. Neither ID nor
    // any Number field is supplied; no display string becomes an edit value.
    let new = harness.get_by_role_and_label(Role::Button, "New");
    assert!(!new.accesskit_node().is_disabled());
    new.click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    let generated = harness.get_by_role_and_label(Role::Label, "(New)");
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::SetValue)
    );
    assert!(
        !generated
            .accesskit_node()
            .data()
            .supports_action(egui::accesskit::Action::Focus)
    );
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "ID")
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::MultilineTextInput, "ID")
            .is_none()
    );
}

pub(in crate::workspace) fn apply_omitted_fields(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    after_column: &[u8],
    schema_revision: u64,
    columns: &[&str],
) {
    begin_generated_insert(harness);
    for column in columns {
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, column)
                .value()
                .as_deref(),
            Some("")
        );
        // Exercise the actual mode menu, then explicitly omit each field. No
        // literal zero, NULL or empty numeric value is submitted.
        choose_field_mode(harness, column, "Omitted");
    }
    review_and_save_insert(harness, path, after_column);
    let reopened = harness.state().state.data.as_ref().unwrap();
    assert_eq!(reopened.table, "Table1");
    assert_ne!(reopened.source_revision, Some(schema_revision));
}

pub(in crate::workspace) fn review_and_save_insert(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
) {
    review_insert(harness);
    let review_id = harness.state().editing.insert_review_id().unwrap();
    assert!(harness.state().state.can_apply_insert(review_id));
    assert_eq!(std::fs::read(path).unwrap(), before, "row review is inert");
    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1)
            && receipt.schema_change.is_none()
            && receipt.query_change.is_none()),
        "{report:?}"
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(path).exists());
}

pub(in crate::workspace) fn insert_omitted_fields(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    after_column: &[u8],
    schema_revision: u64,
    columns: &[&str],
) {
    apply_omitted_fields(harness, path, after_column, schema_revision, columns);
    let reopened = harness.state().state.data.as_ref().unwrap();
    assert_eq!(reopened.rows.len(), 1);
    assert_eq!(
        reopened.rows[0][0].text, "1",
        "the native generator supplies ID"
    );
    assert_eq!(reopened.rows[0].len(), columns.len() + 1);
    for (index, column) in columns.iter().enumerate() {
        assert_eq!(reopened.columns[index + 1].name, *column);
        assert_eq!(
            reopened.rows[0][index + 1].text,
            "0",
            "omitted {column} uses its stored default"
        );
    }
    for cell in &reopened.rows[0] {
        assert!(matches!(cell.kind, CellKind::Scalar { is_null: false, .. }));
    }
}

pub(in crate::workspace) fn choose_field_mode(
    harness: &mut Harness<'_, Desktop>,
    field: &str,
    mode: &str,
) {
    let mut arguments = tablecraft_ui::FluentArgs::new();
    arguments.set("field", field);
    let label = harness
        .state()
        .state
        .localizer
        .format("insertion-field-input", &arguments);
    // Omitted is initially selected, so transition through Value to exercise a
    // real changed selection and close the popup on returning to Omitted.
    if mode == "Omitted" {
        choose_field_mode(harness, field, "Value");
    }
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::RadioButton, mode)
        .click_accesskit();
    harness.run_steps(2);
}
