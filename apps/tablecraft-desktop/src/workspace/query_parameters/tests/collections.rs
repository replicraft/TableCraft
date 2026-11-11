//! Normal SQL Run must expose already-qualified attachment metadata members.
use super::{opened_with_source, run};
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
