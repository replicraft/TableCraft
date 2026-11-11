//! Named attachment members remain scalar query rows without native row authority.
use super::{query, query_parameters::inspect, request, stored::opened};
use crate::{CellKind, Command, DisplayControl, Response};
use tablecraft_testkit::fixtures::access::attachment_modern;

const MEMBERS: &str =
    "SELECT Id, Score.FileName AS FileName, Score.FileFlags AS Flags FROM Items ORDER BY Id";

fn assert_rows(response: Response, expected: &[&[&str]]) {
    let Response::Data {
        source_revision,
        rows,
        has_more,
        ..
    } = response
    else {
        panic!("expected buffered scalar query rows");
    };
    assert_eq!(
        source_revision, None,
        "query results have no native row handles"
    );
    assert!(!has_more);
    assert_eq!(rows.len(), expected.len());
    for (row, expected) in rows.iter().zip(expected) {
        assert_eq!(row.len(), expected.len());
        for (cell, expected) in row.iter().zip(*expected) {
            assert_eq!(cell.text, *expected);
            assert!(matches!(
                cell.kind,
                CellKind::Scalar {
                    is_null,
                    stored_calculation: false,
                    boolean: None,
                    display_control: DisplayControl::Uninspected,
                } if is_null == (*expected == "NULL")
            ));
        }
    }
}

fn assert_members(response: Response) {
    assert_rows(
        response,
        &[
            &["1", "synthetic.bin", "-2147483648"],
            &["1", "NULL", "NULL"],
            &["2", "", "-2147483648"],
            &["3", "NULL", "NULL"],
        ],
    );
}

#[test]
fn direct_attachment_member_query_preserves_scalar_null_and_empty_values() {
    let before = attachment_modern::database(6);
    let (_directory, path, worker, _revision) = opened(&before);
    assert_rows(
        request(&worker, query("SELECT Id FROM Items ORDER BY Id")).unwrap(),
        &[&["1"], &["2"], &["3"]],
    );
    assert_members(request(&worker, query(MEMBERS)).unwrap());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn inspected_attachment_member_query_preserves_scalar_results_and_source_bytes() {
    let before = attachment_modern::database(6);
    let (_directory, path, worker, revision) = opened(&before);
    let (target, parameters) = inspect(&worker, revision, MEMBERS);
    assert!(parameters.is_empty());
    assert_members(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap(),
    );
    assert_eq!(std::fs::read(path).unwrap(), before);
}
