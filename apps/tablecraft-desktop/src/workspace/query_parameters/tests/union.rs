//! Normal SQL View publishes complete UNION results through the existing read service.
use super::{confirm, enter, opened_with_source, run, run_until_terminal};
use crate::workspace::test_support::finish_reads;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::{path::Path, sync::Arc};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::joined_update;
use tablecraft_ui::CellKind;

fn source_unchanged(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first changed source byte"
    );
    assert!(!journal_path(path).exists());
}

#[test]
fn sql_view_union_and_union_all_publish_distinct_null_membership_without_edit_authority() {
    let (_directory, path, before, mut harness) = opened_with_source(joined_update::source().bytes);
    for (operator, expected_each) in [("UNION ALL", 2), ("UNION", 1)] {
        let sql = format!(
            "SELECT Score AS [ResultValue] FROM Table1 {operator} SELECT Score AS [OtherName] FROM Table1"
        );
        run(&mut harness, &sql);
        assert_eq!(harness.state().state.sql, sql);
        assert!(harness.state().query_parameters.is_none());
        let result = harness.state().state.query_data.as_ref().unwrap();
        assert_eq!(result.source_revision, None);
        assert_eq!(result.columns.len(), 1);
        assert_eq!(result.columns[0].name, "ResultValue");
        assert_eq!(result.rows.len(), expected_each * 2);
        assert_eq!(
            result
                .rows
                .iter()
                .filter(|row| matches!(row[0].kind, CellKind::Scalar { is_null: true, .. }))
                .count(),
            expected_each
        );
        assert_eq!(
            result
                .rows
                .iter()
                .filter(
                    |row| matches!(row[0].kind, CellKind::Scalar { is_null: false, .. })
                        && row[0].text == "17"
                )
                .count(),
            expected_each
        );
        let revision = harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision;
        assert!(
            !harness
                .state()
                .state
                .can_edit_cell_at(revision, "Table1", "Score", 0)
        );
        assert!(!harness.state().state.query_results_previous());
        assert!(!harness.state().editing.has_save());
        source_unchanged(&path, &before);
    }
}

#[test]
fn declared_union_parameter_cancel_retains_previous_result_then_run_uses_one_typed_input() {
    let (_directory, path, before, mut harness) = opened_with_source(joined_update::source().bytes);
    run(&mut harness, "SELECT 99 AS Previous");
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    let sql = "PARAMETERS [Needle] LONG; SELECT [Needle] AS [ResultValue] FROM Table1 WHERE ID=1 UNION ALL SELECT [Needle] AS [OtherName] FROM Table1 WHERE ID=2";
    run(&mut harness, sql);
    let target = Arc::downgrade(&harness.state().query_parameters.as_ref().unwrap().target);
    enter(&mut harness, "Needle", "41");
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().query_parameters.is_none());
    assert!(target.upgrade().is_none());
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
    assert!(harness.state().state.error.is_none());
    source_unchanged(&path, &before);

    run(&mut harness, sql);
    enter(&mut harness, "Needle", "7");
    confirm(&mut harness);
    finish_reads(&mut harness);
    assert!(harness.state().query_parameters.is_none());
    let result = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(result.source_revision, None);
    assert_eq!(result.columns[0].name, "ResultValue");
    assert_eq!(result.rows.len(), 2);
    assert!(
        result.rows.iter().all(|row| row[0].text == "7"
            && matches!(row[0].kind, CellKind::Scalar { is_null: false, .. }))
    );
    assert!(!harness.state().state.query_results_previous());
    source_unchanged(&path, &before);
}

#[test]
fn late_union_arm_error_does_not_publish_a_valid_prefix_or_replace_the_previous_result() {
    let (_directory, path, before, mut harness) = opened_with_source(joined_update::source().bytes);
    run(
        &mut harness,
        "SELECT Score AS [ResultValue] FROM Table1 UNION SELECT Score FROM Table1",
    );
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    // The first arm and the first row of the second arm are valid. The final
    // original ID2 must fail before any complete result replaces the old owner.
    let sql = "SELECT ID AS [ResultValue] FROM Table1 UNION ALL SELECT 10 \\ (2-ID) FROM Table1";
    run_until_terminal(&mut harness, sql);
    let report = harness
        .state()
        .state
        .error
        .as_deref()
        .expect("late arm refusal");
    assert!(
        report.contains("division by zero"),
        "expected late evaluation failure: {report}"
    );
    assert!(
        harness.query_all_by_role(Role::Label).any(|node| {
            let node = node.accesskit_node();
            node.label().is_some_and(|text| text == report)
                || node.value().is_some_and(|text| text == report)
        }),
        "actual late failure must be visible"
    );
    assert_eq!(harness.state().state.sql, sql);
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .as_ptr(),
        previous
    );
    assert!(!harness.state().editing.has_save());
    source_unchanged(&path, &before);

    run(&mut harness, "SELECT ID FROM Table1 ORDER BY ID");
    let rows = &harness.state().state.query_data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[1][0].text, "2");
    source_unchanged(&path, &before);
}
