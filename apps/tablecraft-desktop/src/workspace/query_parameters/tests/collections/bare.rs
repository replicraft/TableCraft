//! A bare attachment stays one structured cell on each parent result row.
use super::super::{opened_with_source, run, run_until_terminal};
use super::assert_source;
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
}
