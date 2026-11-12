//! Actual Design Run preserves grouped drafts and uses shared typed parameter prompts.
use super::{design, opened, run, scratch, sql};
use crate::workspace::test_support::finish_reads;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use std::{
    fs,
    time::{Duration, Instant},
};
use tablecraft_builders::query::DesignTotal;
use tablecraft_testkit::fixtures::access::mutation_fixture;

const GROUPED: &str = "PARAMETERS [Minimum] LONG; SELECT Score,Sum(Id) AS Total FROM Items WHERE Id>1 GROUP BY Score HAVING Sum(Id)>=[Minimum] ORDER BY Score";

#[test]
fn grouped_design_run_keeps_typed_prompt_cancel_and_then_publishes_complete_results() {
    let bytes = mutation_fixture::with_rows_and_sort_order(&[(1, 1), (2, 1), (3, 2)], [9, 4, 0, 0]);
    let (_directory, path, original, mut harness) = opened(bytes);
    scratch(&mut harness);
    sql(&mut harness, "SELECT 99 AS Previous");
    run(&mut harness);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    sql(&mut harness, GROUPED);
    design(&mut harness);
    let draft = harness.state().state.query_design_draft().unwrap();
    assert!(draft.totals);
    harness.get_by_role_and_label(Role::ComboBox, "Total: 1");
    harness.get_by_role_and_label(Role::ComboBox, "Total: 2");
    assert_eq!(draft.parameters.len(), 1);
    assert_eq!(draft.parameters[0].name.as_str(), "Minimum");
    assert!(
        draft
            .columns
            .iter()
            .any(|column| column.total == DesignTotal::Sum)
    );
    assert!(
        draft
            .columns
            .iter()
            .any(|column| column.total == DesignTotal::Where)
    );
    run(&mut harness);
    assert!(harness.state().query_parameters.is_some());
    harness.get_by_role_and_label(Role::TextInput, "Minimum");
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
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().query_parameters.is_none());
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
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(harness.state().state.query_design_draft().unwrap().totals);
    run(&mut harness);
    assert!(harness.state().query_parameters.is_some());
    harness
        .get_by_role_and_label(Role::TextInput, "Minimum")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.event(egui::Event::Text("3".into()));
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(2);
    finish_reads(&mut harness);
    assert!(harness.state().query_parameters.is_none());
    let data = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "2");
    assert_eq!(data.rows[0][1].text, "3");
    assert!(data.source_revision.is_none());
    assert!(data.row_ordinals.is_none());
    assert!(!harness.state().state.query_results_previous());
    let draft = harness.state().state.query_design_draft().unwrap();
    assert!(draft.totals);
    assert_eq!(draft.parameters[0].name.as_str(), "Minimum");
    assert!(!harness.state().editing.has_save());
    assert!(!tablecraft_runtime::journal_path(&path).exists());
    assert_eq!(fs::read(path).unwrap(), original);
}

#[test]
fn later_aggregate_error_and_unsupported_import_keep_prior_result_and_literal_sql() {
    let bytes = mutation_fixture::with_rows_and_sort_order(&[(1, 1), (2, 2), (3, 2)], [9, 4, 0, 0]);
    let (_directory, path, original, mut harness) = opened(bytes);
    scratch(&mut harness);
    sql(&mut harness, "SELECT 99 AS Previous");
    run(&mut harness);
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    sql(
        &mut harness,
        "SELECT Score,Sum(1/(Id-3)) AS Total FROM Items GROUP BY Score",
    );
    design(&mut harness);
    assert!(harness.state().state.query_design_draft().unwrap().totals);
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy {
        assert!(Instant::now() < deadline, "grouped Run must terminate");
        harness.run_steps(1);
        std::thread::yield_now();
    }
    harness.run_steps(2);
    let labels = harness
        .get_all_by_role(Role::Label)
        .filter_map(|node| node.value())
        .collect::<Vec<_>>();
    assert!(
        labels
            .iter()
            .any(|label| label.to_lowercase().contains("division by zero")),
        "visible grouped diagnosis: {labels:?}"
    );
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
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "SQL View")
        .click_accesskit();
    harness.run_steps(2);
    let unsupported = "SELECT Id FROM Items UNION SELECT Id FROM Items";
    sql(&mut harness, unsupported);
    design(&mut harness);
    assert_eq!(harness.state().state.sql, unsupported);
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
    assert_eq!(fs::read(path).unwrap(), original);
}
