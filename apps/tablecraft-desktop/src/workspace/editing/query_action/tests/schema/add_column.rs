//! Actual normal Run reviews unadorned ADD instead of forging a gallery request.
use super::{assert_image, finish_reads, finish_save, opened, reviewed};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_runtime::{SaveKnownOutcome, SaveOutcome, SaveSchemaChange, journal_path};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    declared_fields as raw, field_changes::inserting,
};
use tablecraft_ui::ObjectKind;

const SQL: &str = "ALTER TABLE Items ADD COLUMN Amount CURRENCY";

#[test]
fn sql_add_column_no_then_yes_preserves_absence_and_publishes_only_exact_native_after() {
    let mut source = raw::source();
    source.bytes = inserting::keyless_score(&source.bytes, source.definition, None);
    let expected = raw::after(&source.bytes, source.definition, "Amount");
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let original = harness.state().state.active_object().unwrap();
    let prior = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    reviewed(&mut harness, SQL);
    harness.get_by_role_and_label(Role::Label, "Amount");
    harness.get_by_role_and_label(Role::Label, "Currency");
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "No")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.sql, SQL);
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        prior
    );
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!journal_path(&path).exists());
    reviewed(&mut harness, SQL);
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.known == SaveKnownOutcome::Committed
        && receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
        && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.native_source_stale());
    assert!(!journal_path(&path).exists());
    assert_eq!(harness.state().state.sql, SQL);
    let fresh = harness.state().state.active_object().unwrap();
    assert_eq!(fresh.target.kind, ObjectKind::Sql);
    assert!(fresh.target.document_revision > original.target.document_revision);
    assert!(harness.state().state.query_data.is_none());
    super::super::run(&mut harness, "SELECT Score,Amount FROM Items");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows[0][0].text, "23");
    assert!(matches!(
        rows[0][1].kind,
        tablecraft_ui::CellKind::Scalar { is_null: true, .. }
    ));
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    let updated = raw::update_currency(&expected, source.definition, 0, Some(52_500));
    reviewed(
        &mut harness,
        "UPDATE Items SET Amount=CCUR(5.25) WHERE Score=23",
    );
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.known == SaveKnownOutcome::Committed
        && receipt.matched_rows == Some(1) && receipt.changed_rows == Some(1)
        && receipt.schema_change.is_none()));
    assert_image(&std::fs::read(&path).unwrap(), &updated);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());
    super::super::run(&mut harness, "SELECT Score,Amount FROM Items");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "23");
    assert!(matches!(
        rows[0][1].kind,
        tablecraft_ui::CellKind::Scalar { is_null: false, .. }
    ));
    assert!(rows[1].iter().all(|cell| matches!(
        cell.kind,
        tablecraft_ui::CellKind::Scalar { is_null: true, .. }
    )));
    let expected = updated;
    let inserted = raw::append_omitted(&expected, source.definition);
    reviewed(&mut harness, "INSERT INTO Items (Score) VALUES (NULL)");
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Yes")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.known == SaveKnownOutcome::Committed
        && receipt.changed_rows == Some(1) && receipt.schema_change.is_none()));
    assert_image(&std::fs::read(&path).unwrap(), &inserted);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());
    super::super::run(&mut harness, "SELECT Score,Amount FROM Items");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0][0].text, "23");
    assert!(rows[1].iter().all(|cell| matches!(
        cell.kind,
        tablecraft_ui::CellKind::Scalar { is_null: true, .. }
    )));
    assert_image(&std::fs::read(&path).unwrap(), &inserted);
    assert!(rows[2].iter().all(|cell| matches!(
        cell.kind,
        tablecraft_ui::CellKind::Scalar { is_null: true, .. }
    )));
}
