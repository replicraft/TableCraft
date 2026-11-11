//! Normal SQL Run must expose already-qualified attachment metadata members.
mod bare;
use super::{opened_with_source, run, run_until_terminal};
use egui::{Key, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::attachment_modern;
use tablecraft_ui::CellKind;

#[test]
fn sql_view_attachment_metadata_members_preserve_null_empty_and_child_alignment() {
    let (_directory, path, before, mut harness) =
        opened_with_source(attachment_modern::database(6));
    run(&mut harness, "SELECT Id FROM Items ORDER BY Id");
    assert!(harness.state().state.error.is_none());
    let control = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(control.columns.len(), 1);
    assert_eq!(
        control
            .rows
            .iter()
            .map(|row| row[0].text.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    assert_source(&path, &before);

    let sql =
        "SELECT Id, Score.FileName AS FileName, Score.FileFlags AS Flags FROM Items ORDER BY Id";
    run(&mut harness, sql);
    // Check the physical source even when the current scalar-only runtime
    // refuses the intended query. A read failure must never modify its input.
    assert_source(&path, &before);
    assert_eq!(harness.state().state.sql, sql);
    assert!(!harness.state().state.busy);
    assert!(harness.state().query_parameters.is_none());
    assert!(
        harness.state().state.error.is_none(),
        "normal SQL Run refused qualified attachment metadata: {:?}",
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
        ["Id", "FileName", "Flags"]
    );
    let values = result
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    // Parent 1 has two aligned children, including an actual NULL metadata row.
    // Parent 2 has present empty Text. Parent 3 has no children and contributes
    // one NULL-extended row, rather than disappearing or inventing empty Text.
    assert_eq!(
        values,
        [
            vec!["1", "synthetic.bin", "-2147483648"],
            vec!["1", "NULL", "NULL"],
            vec!["2", "", "-2147483648"],
            vec!["3", "NULL", "NULL"],
        ]
    );
    for row in [1, 3] {
        for column in [1, 2] {
            assert!(matches!(
                result.rows[row][column].kind,
                CellKind::Scalar { is_null: true, .. }
            ));
        }
    }
    assert!(matches!(
        result.rows[2][1].kind,
        CellKind::Scalar { is_null: false, .. }
    ));
    assert_source(&path, &before);

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
    assert!(
        !harness
            .state()
            .state
            .can_view_contents(&tablecraft_ui::ContentsTarget {
                document_revision: revision,
                table: "Query results".into(),
                column: "FileName".into(),
                parent_ordinal: 0,
            })
    );
    let cell = harness
        .query_all_by_role(Role::Label)
        .find(|node| {
            let node = node.accesskit_node();
            node.label()
                .is_some_and(|text| text.contains("synthetic.bin"))
                || node
                    .value()
                    .is_some_and(|text| text.contains("synthetic.bin"))
        })
        .expect("actual published query result cell");
    cell.click_secondary();
    harness.run_steps(2);
    let edit_label = harness.state().state.localizer.text("datasheet-edit-cell");
    let contents_label = harness
        .state()
        .state
        .localizer
        .text("datasheet-view-contents");
    assert!(
        harness
            .query_by_role_and_label(Role::Button, &edit_label)
            .is_none()
    );
    assert!(
        harness
            .query_by_role_and_label(Role::Button, &contents_label)
            .is_none()
    );
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert!(!harness.state().state.edit_dialog_open());
    assert!(harness.state().state.contents_request_id().is_none());
    assert_source(&path, &before);

    // The whole child recordset has no scalar string conversion. Refusal must
    // retain the last fully published result, without stringifying children,
    // publishing a partial projection or fabricating native row identity.
    run_until_terminal(
        &mut harness,
        "SELECT Id, CStr(Score) AS Converted FROM Items ORDER BY Id",
    );
    let report = harness
        .state()
        .state
        .error
        .as_deref()
        .expect("whole-recordset scalar conversion refusal");
    assert!(
        harness.query_all_by_role(Role::Label).any(|node| {
            let node = node.accesskit_node();
            node.label().is_some_and(|text| text == report)
                || node.value().is_some_and(|text| text == report)
        }),
        "the actual refusal must be visible while the previous result stays published"
    );
    let retained = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(retained.source_revision, None);
    assert_eq!(
        retained
            .columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "FileName", "Flags"]
    );
    assert_eq!(
        retained
            .rows
            .iter()
            .map(|row| row
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [
            vec!["1", "synthetic.bin", "-2147483648"],
            vec!["1", "NULL", "NULL"],
            vec!["2", "", "-2147483648"],
            vec!["3", "NULL", "NULL"],
        ]
    );
    assert!(!harness.state().state.busy);
    assert!(!harness.state().state.edit_dialog_open());
    assert!(harness.state().state.contents_request_id().is_none());
    assert_source(&path, &before);
}

fn assert_source(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    let first = actual
        .iter()
        .zip(expected)
        .position(|(left, right)| left != right);
    assert!(
        actual.len() == expected.len() && first.is_none(),
        "read-only source changed: actual length {}, expected length {}, first differing byte {:?}",
        actual.len(),
        expected.len(),
        first
    );
    assert!(!journal_path(path).exists());
}
