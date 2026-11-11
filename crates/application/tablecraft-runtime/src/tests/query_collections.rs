//! Named attachment members remain scalar query rows without native row authority.
use super::{query, query_parameters::inspect, request, stored::opened};
use crate::{CellKind, Command, DisplayControl, Response};
use tablecraft_testkit::fixtures::access::attachment_modern;

const MEMBERS: &str =
    "SELECT Id, Score.FileName AS FileName, Score.FileFlags AS Flags FROM Items ORDER BY Id";

fn assert_source(path: &std::path::Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    assert_eq!(actual.len(), expected.len(), "source image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing source byte"
    );
}

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
    assert_source(&path, &before);
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
    assert_source(&path, &before);
}

#[test]
fn bare_collections_refuse_scalar_publication_without_poisoning_explicit_members() {
    use tablecraft_query::QueryError;
    use tablecraft_testkit::fixtures::access::complex_links;

    let before = attachment_modern::database(6);
    let (_directory, path, worker, _revision) = opened(&before);
    let failure = request(&worker, query("SELECT Score FROM Items")).err();
    assert!(
        matches!(failure, Some(crate::Error::Query(QueryError::UnknownName))),
        "bare attachment has no fabricated scalar binding: {failure:?}"
    );
    assert_members(request(&worker, query(MEMBERS)).unwrap());
    assert_source(&path, &before);

    let before = complex_links::populated();
    let (_directory, path, worker, _revision) = opened(&before);
    let failure = request(&worker, query("SELECT Score FROM Items")).err();
    assert!(
        matches!(failure, Some(crate::Error::Query(QueryError::Unsupported))),
        "a complete multivalue list cannot become scalar text: {failure:?}"
    );
    assert!(request(&worker, query("SELECT Score.Value FROM Items")).is_ok());
    assert_source(&path, &before);
}

#[test]
fn attachment_member_budgets_refuse_without_partial_data_or_cached_prefixes() {
    use crate::{Error, QueryPoolLimits, QueryRunOptions, QueryRunPool};
    use tablecraft_core::limits::Resource;
    use tablecraft_query::QueryError;

    let before = attachment_modern::database(6);
    let (_directory, path, worker, _revision) = opened(&before);
    let defaults = QueryRunOptions::default();
    for native_pool in [true, false] {
        let small = QueryPoolLimits::new(1, 10_000, 1_000_000).unwrap();
        let options = QueryRunOptions::new(
            defaults.snapshot(),
            if native_pool {
                small
            } else {
                defaults.native()
            },
            if native_pool { defaults.query() } else { small },
            defaults.comparison(),
            defaults.presentation(),
        )
        .unwrap();
        let failure = request(
            &worker,
            Command::Query {
                sql: MEMBERS.to_owned(),
                options,
            },
        )
        .err();
        if native_pool {
            assert!(
                matches!(
                    failure,
                    Some(Error::ReadQueryLimit {
                        pool: QueryRunPool::Native,
                        source: tablecraft_core::limits::LimitError {
                            resource: Resource::Bytes,
                            ..
                        },
                    })
                ),
                "native allowance: {failure:?}"
            );
        } else {
            assert!(
                matches!(failure, Some(Error::Query(QueryError::Limit(_)))),
                "query allowance: {failure:?}"
            );
        }
        assert_members(request(&worker, query(MEMBERS)).unwrap());
    }
    assert_source(&path, &before);
}

#[test]
fn inspected_attachment_members_bind_parameters_and_reject_reopened_source_before_scans() {
    let before = attachment_modern::database(6);
    let (_directory, path, worker, revision) = opened(&before);
    let (target, parameters) = inspect(
        &worker,
        revision,
        "PARAMETERS Minimum LONG; SELECT Id,Count(Score.FileName) AS N FROM Items WHERE Id>=Minimum GROUP BY Id ORDER BY Id",
    );
    assert_eq!(parameters.len(), 1);
    assert_rows(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target,
                parameters: vec![crate::ParameterInput {
                    name: "Minimum".to_owned(),
                    kind: "int32".to_owned(),
                    text: "2".to_owned(),
                    is_null: false,
                }],
            },
        )
        .unwrap(),
        &[&["2", "1"], &["3", "0"]],
    );
    let (target, _) = inspect(&worker, revision, MEMBERS);
    let mut unreadable_children = before;
    unreadable_children[22 * 4096] = 0;
    std::fs::write(&path, &unreadable_children).unwrap();
    request(&worker, super::open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target,
                parameters: Vec::new(),
            },
        ),
        Err(crate::Error::StaleQuery)
    ));
    assert_source(&path, &unreadable_children);
}

#[test]
fn attachment_file_data_remains_encoded_binary_including_invalid_empty_wrappers() {
    use tablecraft_testkit::fixtures::access::attachments;

    let mut before = attachments::database();
    before[20] = 6;
    let (_directory, path, worker, _revision) = opened(&before);
    let encoded = attachments::PAYLOAD
        .iter()
        .fold(String::from("0x"), |mut text, byte| {
            use std::fmt::Write;
            write!(&mut text, "{byte:02x}").unwrap();
            text
        });
    assert_rows(
        request(
            &worker,
            query("SELECT Id,Score.FileData FROM Items ORDER BY Id"),
        )
        .unwrap(),
        &[
            &["1", &encoded],
            &["1", "NULL"],
            &["2", "0x"],
            &["3", "NULL"],
        ],
    );
    assert_source(&path, &before);
}
