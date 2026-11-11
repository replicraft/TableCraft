//! A bare attachment stays one structured cell on each parent result row.
use super::super::{opened_with_source, run, run_until_terminal};
use super::assert_source;
use crate::workspace::test_support::finish_reads;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_testkit::fixtures::access::attachment_modern;
use tablecraft_ui::CellKind;

#[test]
fn sql_view_attachment_wildcard_keeps_parent_rows_and_readonly_collection_summaries() {
    let (_directory, path, before, mut harness) =
        opened_with_source(attachment_modern::database(6));
    run(&mut harness, "SELECT Id FROM Items ORDER BY Id");
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .map(|row| row[0].text.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    assert_source(&path, &before);

    let sql = "SELECT * FROM Items ORDER BY Id";
    run_until_terminal(&mut harness, sql);
    assert_source(&path, &before);
    assert_eq!(harness.state().state.sql, sql);
    assert!(!harness.state().state.busy);
    assert!(harness.state().query_parameters.is_none());
    assert!(
        harness.state().state.error.is_none(),
        "normal SQL Run refused the parent-associated attachment wildcard: {:?}",
        harness.state().state.error
    );
    let result = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(result.source_revision, None);
    assert_eq!(
        result
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    assert_eq!(
        result.rows.len(),
        3,
        "bare attachments must not flatten child rows"
    );
    for (row, (id, members, summary)) in result.rows.iter().zip([
        ("1", 2, "2 attachments"),
        ("2", 1, "1 attachment"),
        ("3", 0, "0 attachments"),
    ]) {
        assert_eq!(row.len(), 2);
        assert_eq!(row[0].text, id);
        assert!(matches!(
            row[0].kind,
            CellKind::Scalar { is_null: false, .. }
        ));
        assert_eq!(row[1].text, summary);
        assert!(
            row[1].kind
                == CellKind::Attachments {
                    assigned: true,
                    members
                },
            "parent {id} must retain its assigned attachment membership count {members}"
        );
    }
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
            .can_edit_cell_at(revision, "Items", "Score", 0)
    );
    assert!(!harness.state().state.edit_dialog_open());
    assert!(harness.state().state.contents_request_id().is_none());
    assert_source(&path, &before);

    let cell = harness
        .query_all_by_role(Role::Label)
        .find(|node| {
            let node = node.accesskit_node();
            node.label()
                .is_some_and(|text| text.contains("2 attachments"))
                || node
                    .value()
                    .is_some_and(|text| text.contains("2 attachments"))
        })
        .expect("actual parent-associated attachment summary");
    cell.click_secondary();
    harness.run_steps(2);
    let edit = harness.state().state.localizer.text("datasheet-edit-cell");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, &edit)
            .is_none()
    );
    let contents = harness
        .state()
        .state
        .localizer
        .text("datasheet-view-contents");
    harness
        .get_by_role_and_label(Role::Button, &contents)
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(harness.state().state.contents_request_id().is_some());
    assert!(!harness.state().state.edit_dialog_open());
    assert!(
        harness.query_all_by_role(Role::Label).any(|node| {
            let node = node.accesskit_node();
            node.label()
                .is_some_and(|text| text.contains("FileData") && text.contains("Not inspected"))
                || node
                    .value()
                    .is_some_and(|text| text.contains("FileData") && text.contains("Not inspected"))
        }),
        "FileData must remain visibly uninspected, not NULL or decoded payload"
    );
    assert_source(&path, &before);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().state.contents_request_id().is_none());
    assert_eq!(
        harness
            .state()
            .state
            .query_data
            .as_ref()
            .unwrap()
            .rows
            .len(),
        3
    );
    assert_source(&path, &before);
}
