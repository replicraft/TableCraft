//! Real SQL View and saved native crosstabs use the shared read/prompt pipeline.
use super::{confirm, enter, opened, opened_with_source, run, run_until_terminal};
use crate::workspace::test_support::finish_reads;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::saved_queries::{
    Fragment, QUERY_ID, database_with_query_flags,
};
use tablecraft_ui::{CellKind, Data, ObjectKind, ObjectTarget};

const SQL: &str = "PARAMETERS Floor LONG; TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket, Count(*) AS Total FROM Items WHERE Id >= Floor GROUP BY 0 ORDER BY Bucket PIVOT Id IN (3,1,4)";

fn unchanged(path: &Path, before: &[u8]) {
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(!journal_path(path).exists());
}

fn headings(data: &Data) {
    assert_eq!(
        data.columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Bucket", "Total", "3", "1", "4"]
    );
}

fn floor_two(data: &Data) {
    headings(data);
    assert_eq!(data.rows.len(), 1);
    let row = &data.rows[0];
    assert_eq!(row[0].text, "0");
    assert_eq!(row[1].text, "2");
    assert_eq!(row[2].text, "1");
    for cell in &row[3..] {
        assert!(matches!(cell.kind, CellKind::Scalar { is_null: true, .. }));
    }
}

#[test]
fn sql_view_crosstab_prompts_cancel_and_empty_results_keep_literal_heading_order() {
    let (_directory, path, before, mut harness) = opened();
    run(&mut harness, "SELECT 99 AS Previous");
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    run(&mut harness, SQL);
    assert!(harness.state().state.parameter_prompt_open());
    enter(&mut harness, "Floor", "2");
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.parameter_prompt_open());
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
    unchanged(&path, &before);

    run(&mut harness, SQL);
    enter(&mut harness, "Floor", "2");
    confirm(&mut harness);
    finish_reads(&mut harness);
    floor_two(harness.state().state.query_data.as_ref().unwrap());
    assert!(!harness.state().state.query_results_previous());
    assert!(!harness.state().editing.has_save());

    run(&mut harness, SQL);
    enter(&mut harness, "Floor", "9");
    confirm(&mut harness);
    finish_reads(&mut harness);
    let result = harness.state().state.query_data.as_ref().unwrap();
    headings(result);
    assert!(result.rows.is_empty());
    unchanged(&path, &before);
}

#[test]
fn late_transform_aggregate_error_preserves_the_complete_previous_result() {
    let (_directory, path, before, mut harness) = opened();
    run(
        &mut harness,
        "TRANSFORM Count(*) AS Measure SELECT 0 AS Bucket, Count(*) AS Total FROM Items GROUP BY 0 PIVOT Id IN (3,1,4)",
    );
    let previous = harness
        .state()
        .state
        .query_data
        .as_ref()
        .unwrap()
        .rows
        .as_ptr();
    // ID1 contributes successfully; ID2 fails before publication of the pivot.
    run_until_terminal(
        &mut harness,
        "TRANSFORM Sum(10 \\ (2-Id)) AS Measure SELECT 0 AS Bucket FROM Items GROUP BY 0 PIVOT Id IN (3,1,4)",
    );
    let error = harness
        .state()
        .state
        .error
        .as_deref()
        .expect("aggregate failure");
    assert!(
        error.contains("division by zero"),
        "unexpected failure: {error}"
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
    assert!(!harness.state().editing.has_save());
    unchanged(&path, &before);
}

fn fragment(attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }
}

fn native_crosstab() -> Vec<u8> {
    database_with_query_flags(
        &[
            Fragment {
                flag: Some(6),
                ..fragment(1)
            },
            Fragment {
                name1: Some("Floor"),
                flag: Some(4),
                ..fragment(2)
            },
            Fragment {
                name1: Some("Items"),
                ..fragment(5)
            },
            Fragment {
                expression: Some("0"),
                name1: Some("Bucket"),
                flag: Some(2),
                ..fragment(6)
            },
            Fragment {
                expression: Some("Count(*)"),
                name1: Some("Total"),
                flag: Some(2),
                order: Some(&[0, 0, 0, 2]),
                ..fragment(6)
            },
            Fragment {
                expression: Some("Count(*)"),
                name1: Some("Measure"),
                flag: Some(0),
                order: Some(&[0, 0, 0, 3]),
                ..fragment(6)
            },
            Fragment {
                expression: Some("Id IN (3,1,4)"),
                flag: Some(1),
                order: Some(&[0, 0, 0, 4]),
                ..fragment(6)
            },
            Fragment {
                expression: Some("Id >= Floor"),
                ..fragment(8)
            },
            Fragment {
                expression: Some("0"),
                flag: Some(2),
                ..fragment(9)
            },
            Fragment {
                expression: Some("Id"),
                flag: Some(1),
                order: Some(&[0, 0, 0, 2]),
                ..fragment(9)
            },
            Fragment {
                expression: Some("Bucket"),
                ..fragment(11)
            },
        ],
        0x10,
    )
}

#[test]
fn saved_native_crosstab_inspects_inertly_then_runs_with_its_own_parameters_and_results() {
    let (_directory, path, before, mut harness) = opened_with_source(native_crosstab());
    run(&mut harness, "SELECT 99 AS Scratch");
    let scratch_tab = harness.state().state.active_object().unwrap();
    let database = harness.state().state.database.as_ref().unwrap();
    let saved = database
        .queries
        .iter()
        .find(|query| query.name == "Saved")
        .unwrap();
    // NO_CLONE: The target owns a String while the catalog must remain intact
    // for the later tab checks; this API cannot accept a borrow, Arc or Cow.
    let target = ObjectTarget {
        document_revision: database.document_revision,
        kind: ObjectKind::SavedQuery {
            object_id: saved.object_id,
            name: saved.name.clone(),
        },
    };
    harness.state_mut().open_object(target);
    finish_reads(&mut harness);
    let saved_tab = harness.state().state.active_object().unwrap();
    assert!(harness.state().state.selected_cell_text().is_none());
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Scratch");
    unchanged(&path, &before);

    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().state.parameter_prompt_open());
    enter(&mut harness, "Floor", "2");
    confirm(&mut harness);
    finish_reads(&mut harness);
    floor_two(harness.state().state.data.as_ref().unwrap());
    assert!(harness.state().state.query_data.is_none());
    let previous = harness.state().state.data.as_ref().unwrap().rows.as_ptr();
    harness
        .get_by_role_and_label(Role::Button, "Run saved query")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Floor")
            .value()
            .as_deref(),
        Some("2")
    );
    enter(&mut harness, "Floor", "9");
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows.as_ptr(),
        previous
    );

    harness.state_mut().activate_object(scratch_tab);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.query_data.as_ref().unwrap().rows[0][0].text,
        "99"
    );
    harness.state_mut().activate_object(saved_tab);
    finish_reads(&mut harness);
    floor_two(harness.state().state.data.as_ref().unwrap());
    assert!(!harness.state().editing.has_save());
    unchanged(&path, &before);
}
