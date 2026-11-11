//! Saved SELECT publication retains the same issued metadata as scratch SQL Run.
use super::{inspect, opened, row};
use crate::tests::{open, request};
use crate::{CellKind, Command, Error, Response};
use tablecraft_testkit::fixtures::access::saved_queries::{Fragment, attachments};

#[test]
fn saved_attachment_wildcard_issues_readonly_contents_and_rejects_reopened_source() {
    let before = attachments::database_with(&[
        Fragment {
            name1: Some("Items"),
            ..row(5)
        },
        Fragment {
            expression: Some("*"),
            ..row(6)
        },
    ]);
    let (_directory, path, worker, target) = opened(&before);
    assert!(inspect(&worker, &target).is_empty());
    let Response::Data {
        source_revision,
        table,
        columns,
        rows,
        query_contents,
        has_more,
        ..
    } = request(
        &worker,
        Command::RunSavedQuery {
            options: Default::default(),
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap()
    else {
        panic!("actual saved retrieval publication");
    };
    assert_eq!(table, "Saved");
    assert_eq!(source_revision, None);
    assert!(!has_more);
    assert_eq!(
        columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["Id", "Score"]
    );
    assert_eq!(rows.len(), 3);
    for (position, (row, members)) in rows.iter().zip([2, 1, 0]).enumerate() {
        assert_eq!(row[0].text, (position + 1).to_string());
        assert!(
            row[1].kind
                == CellKind::Attachments {
                    assigned: true,
                    members
                }
        );
    }
    let result = query_contents.expect("saved query-issued immutable metadata owner");
    assert_eq!(
        result
            .bindings()
            .map(|binding| (binding.row, binding.column, binding.slot))
            .collect::<Vec<_>>(),
        [(0, 1, 0), (1, 1, 1), (2, 1, 2)]
    );
    let Response::QueryContents { columns, rows, .. } = request(
        &worker,
        Command::InspectQueryCollection {
            result: result.clone(),
            binding: 0,
        },
    )
    .unwrap() else {
        panic!("saved query child metadata");
    };
    let name = columns
        .iter()
        .position(|column| column.name == "FileName")
        .unwrap();
    let data = columns
        .iter()
        .position(|column| column.name == "FileData")
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][name].text, "synthetic.bin");
    assert!(matches!(
        rows[1][name].kind,
        CellKind::Scalar { is_null: true, .. }
    ));
    assert!(
        rows.iter()
            .all(|row| row[data].kind == CellKind::Uninspected)
    );
    let Response::QueryContents { rows, .. } = request(
        &worker,
        Command::InspectQueryCollection {
            result: result.clone(),
            binding: 2,
        },
    )
    .unwrap() else {
        panic!("assigned empty saved-query collection");
    };
    assert!(rows.is_empty());

    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::InspectQueryCollection { result, binding: 0 }
        ),
        Err(Error::StaleDocument)
    ));
    let actual = std::fs::read(path).unwrap();
    assert_eq!(actual.len(), before.len());
    assert_eq!(
        actual
            .iter()
            .zip(&before)
            .position(|(left, right)| left != right),
        None,
        "first differing source byte"
    );
}
