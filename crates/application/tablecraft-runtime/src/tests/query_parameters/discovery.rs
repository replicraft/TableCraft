use super::{CountingClock, input, inspect, opened, request, rows};
use crate::{Command, Error, QueryRunOptions, Response, Worker};
use std::sync::{Arc, atomic::Ordering};
use tablecraft_testkit::fixtures::access::mutation_fixture;

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn explicit_discovery_reads_metadata_without_touching_broken_user_records_or_clock() {
    let original = mutation_fixture::with_invalid_row_page();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("metadata.mdb");
    std::fs::write(&path, &original).unwrap();
    let clock = Arc::new(CountingClock::default());
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    let revision = opened(&worker, &path);
    let (target, declared) = inspect(
        &worker,
        revision,
        "SELECT Id,Now() FROM Items WHERE Score>[Enter minimum:]",
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
        panic!("expected explicit metadata-only discovery");
    };
    assert!(target.has_implicit_mode());
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name, "Enter minimum:");
    assert_eq!(parameters[0].kind, "value");
    assert_eq!(parameters[0].label, "VALUE");
    assert_eq!(clock.0.load(Ordering::Acquire), 0);
    assert_image(&std::fs::read(&path).unwrap(), &original);
}

#[test]
fn issued_discovery_runs_selected_value_and_rejects_stale_source_before_input() {
    let original = mutation_fixture::database();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("prompt.mdb");
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let revision = opened(&worker, &path);
    let (target, declared) = inspect(
        &worker,
        revision,
        "SELECT [Enter label:] AS Prompt,Id FROM Items WHERE Id=1",
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
        panic!("expected discovered prompt");
    };
    assert_eq!(parameters.len(), 1);
    let values = rows(
        request(
            &worker,
            Command::RunDiscoveredQuery {
                options: QueryRunOptions::default(),
                target: Arc::clone(&target),
                parameters: vec![input("Enter label:", "text", "日本語 🦀")],
            },
        )
        .unwrap(),
    );
    assert_eq!(values.len(), 1);
    assert_eq!(values[0][0].text, "日本語 🦀");
    assert_eq!(values[0][1].text, "1");
    opened(&worker, &path);
    assert!(matches!(
        request(
            &worker,
            Command::RunDiscoveredQuery {
                options: QueryRunOptions::default(),
                target,
                parameters: vec![input("wrong", "unknown", "invalid")]
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert_image(&std::fs::read(&path).unwrap(), &original);
    assert!(!crate::journal_path(&path).exists());
}

#[test]
fn captured_prepared_collection_results_keep_the_real_issuer_and_stale_binding_guards() {
    use crate::CellKind;
    use tablecraft_testkit::fixtures::access::attachment_modern;
    let original = attachment_modern::presence_cases(6);
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("collections.accdb");
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let revision = opened(&worker, &path);
    let (target, _) = inspect(
        &worker,
        revision,
        "SELECT Id,Score,[Enter label:] AS Label FROM Items ORDER BY Id",
    );
    let Response::QueryParametersDiscovered { target, parameters } = request(
        &worker,
        Command::DiscoverQueryParameters {
            target,
            options: QueryRunOptions::default(),
        },
    )
    .unwrap() else {
        panic!("expected collection query metadata");
    };
    assert!(target.has_implicit_mode());
    assert_eq!(parameters.len(), 1);
    let Response::Data {
        source_revision,
        rows,
        query_contents,
        ..
    } = request(
        &worker,
        Command::RunDiscoveredQuery {
            target,
            options: QueryRunOptions::default(),
            parameters: vec![input("Enter label:", "text", "metadata")],
        },
    )
    .unwrap()
    else {
        panic!("expected complete structured publication");
    };
    assert!(source_revision.is_none());
    assert_eq!(rows.len(), 4);
    for (index, (assigned, members)) in [(true, 2), (true, 1), (true, 0), (false, 0)]
        .into_iter()
        .enumerate()
    {
        assert!(rows[index][1].kind == CellKind::Attachments { assigned, members });
        assert_eq!(rows[index][2].text, "metadata");
    }
    let result = query_contents.expect("actual result-local metadata issuer");
    assert_eq!(result.bindings().count(), 4);
    let Response::QueryContents { columns, rows, .. } = request(
        &worker,
        Command::InspectQueryCollection {
            result: result.clone(),
            binding: 0,
        },
    )
    .unwrap() else {
        panic!("expected read-only child metadata");
    };
    let data = columns
        .iter()
        .position(|column| column.name == "FileData")
        .unwrap();
    assert!(
        rows.iter()
            .all(|row| row[data].kind == CellKind::Uninspected)
    );
    opened(&worker, &path);
    assert!(matches!(
        request(
            &worker,
            Command::InspectQueryCollection { result, binding: 0 }
        ),
        Err(Error::StaleDocument)
    ));
    assert_image(&std::fs::read(&path).unwrap(), &original);
}

#[test]
fn runtime_strict_mode_preserves_existing_domain_and_crosstab_execution() {
    let original = mutation_fixture::database();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("strict.mdb");
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let revision = opened(&worker, &path);
    for sql in [
        "SELECT DCount('*','Items') AS Total",
        "TRANSFORM Count(*) SELECT Id FROM Items GROUP BY Id PIVOT Score IN (10,20,30)",
    ] {
        let (target, _) = inspect(&worker, revision, sql);
        let Response::QueryParametersDiscovered { target, parameters } = request(
            &worker,
            Command::DiscoverQueryParameters {
                target,
                options: QueryRunOptions::default(),
            },
        )
        .unwrap() else {
            panic!("expected explicit strict route");
        };
        assert!(!target.has_implicit_mode());
        assert!(parameters.is_empty());
        let values = rows(
            request(
                &worker,
                Command::RunDiscoveredQuery {
                    target,
                    options: QueryRunOptions::default(),
                    parameters: Vec::new(),
                },
            )
            .unwrap(),
        );
        if sql.starts_with("SELECT") {
            assert_eq!(values[0][0].text, "3");
        } else {
            assert_eq!(values.len(), 3);
        }
    }
    assert_image(&std::fs::read(&path).unwrap(), &original);
}
