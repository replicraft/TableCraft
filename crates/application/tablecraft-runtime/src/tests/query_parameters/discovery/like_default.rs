//! Issued implicit parameters share the ordinary SQL View wildcard default.
use super::{input, inspect, opened, request, rows};
use crate::{Command, QueryRunOptions, Response, Worker};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields;

#[test]
fn discovered_like_parameter_uses_ansi89_for_absent_native_mode_without_source_changes() {
    let original = fields::source(fields::Case::FirstNumber).bytes;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("implicit-like.accdb");
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let revision = opened(&worker, &path);
    let (target, declared) = inspect(
        &worker,
        revision,
        "SELECT ID FROM Table1 WHERE 'Apple' LIKE [Pattern:] ORDER BY ID",
    );
    assert!(declared.is_empty());
    let Response::QueryParametersDiscovered { target, parameters } = request(
        &worker,
        Command::DiscoverQueryParameters {
            target,
            options: QueryRunOptions::default(),
        },
    )
    .unwrap() else {
        panic!("explicit metadata discovery");
    };
    assert!(target.has_implicit_mode());
    assert_eq!(parameters.len(), 1);
    for (pattern, expected) in [("a*", 2), ("a%", 0)] {
        let actual = rows(
            request(
                &worker,
                Command::RunDiscoveredQuery {
                    target: std::sync::Arc::clone(&target),
                    parameters: vec![input("Pattern:", "text", pattern)],
                    options: QueryRunOptions::default(),
                },
            )
            .unwrap(),
        );
        assert_eq!(actual.len(), expected);
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}
