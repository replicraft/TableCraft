//! Explicit native saved-query requests through the actual retained worker.
use super::{open, request};
use crate::{Command, Error, ParameterInput, Response, SavedQueryTarget, Worker};
use std::path::PathBuf;
use tablecraft_testkit::fixtures::access::saved_queries::{
    Fragment, QUERY_ID, database_with_query_flags,
};

fn row(attribute: u8) -> Fragment<'static> {
    Fragment {
        object: Some(QUERY_ID),
        attribute: Some(attribute),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }
}

fn column(expression: &str) -> Fragment<'_> {
    Fragment {
        expression: Some(expression),
        ..row(6)
    }
}

fn opened(bytes: &[u8]) -> (tempfile::TempDir, PathBuf, Worker, SavedQueryTarget) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved.accdb");
    std::fs::write(&path, bytes).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision,
        queries,
        ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected inert inventory");
    };
    let saved = queries.iter().find(|query| query.name == "Saved").unwrap();
    assert!(!saved.system);
    let target = SavedQueryTarget {
        document_revision,
        object_id: saved.object_id,
        name: saved.name.clone(),
    };
    (directory, path, worker, target)
}

fn inspect(worker: &Worker, target: &SavedQueryTarget) -> Vec<crate::ParameterSpec> {
    let Response::SavedQuery {
        target: actual,
        parameters,
    } = request(
        worker,
        Command::InspectSavedQuery {
            target: target.clone(),
        },
    )
    .unwrap()
    else {
        panic!("expected inert declarations");
    };
    assert_eq!(actual, *target);
    parameters
}

#[test]
fn inspection_does_not_decode_user_records_or_claim_execution_readiness() {
    let mut bytes = database_with_query_flags(
        &[
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
            column("Id"),
        ],
        0,
    );
    // The user table is deliberately unreadable. Catalog/query inspection is inert.
    bytes[12 * 4096] = 0;
    let (_directory, path, worker, target) = opened(&bytes);
    assert!(inspect(&worker, &target).is_empty());
    assert!(
        request(
            &worker,
            Command::RunSavedQuery {
                target,
                parameters: Vec::new(),
            }
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn typed_declared_parameters_execute_original_fragments_without_native_cell_capabilities() {
    let bytes = database_with_query_flags(
        &[
            Fragment {
                name1: Some("Enter minimum?"),
                flag: Some(4),
                ..row(2)
            },
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
            column("Id"),
            Fragment {
                expression: Some("Id >= [Enter minimum?]"),
                ..row(8)
            },
        ],
        0,
    );
    let (_directory, path, worker, target) = opened(&bytes);
    let specs = inspect(&worker, &target);
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].name, "Enter minimum?");
    assert_eq!(specs[0].kind, "int32");
    assert_eq!(specs[0].label, "LONG");
    assert!(specs[0].nullable);
    let input = ParameterInput {
        name: specs[0].name.clone(),
        kind: specs[0].kind.clone(),
        text: "2".into(),
        is_null: false,
    };
    let Response::Data {
        source_revision,
        table,
        rows,
        has_more,
        ..
    } = request(
        &worker,
        Command::RunSavedQuery {
            target: target.clone(),
            parameters: vec![input.clone()],
        },
    )
    .unwrap()
    else {
        panic!("expected selected saved results");
    };
    assert_eq!(source_revision, None);
    assert_eq!(table, "Saved");
    assert_eq!(
        rows.iter()
            .map(|row| row[0].text.as_str())
            .collect::<Vec<_>>(),
        ["2", "3"]
    );
    assert!(!has_more);
    for parameters in [
        Vec::new(),
        vec![ParameterInput {
            kind: "text".into(),
            ..input.clone()
        }],
        vec![ParameterInput {
            name: "Other".into(),
            ..input.clone()
        }],
        vec![input.clone(), input],
    ] {
        assert!(matches!(
            request(
                &worker,
                Command::RunSavedQuery {
                    target: target.clone(),
                    parameters,
                }
            ),
            Err(Error::ParameterInput)
        ));
    }
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn value_parameters_require_a_carrier_and_literal_text_never_becomes_sql() {
    let bytes = database_with_query_flags(
        &[
            Fragment {
                name1: Some("Prompt"),
                flag: Some(0),
                ..row(2)
            },
            column("[Prompt]"),
        ],
        0,
    );
    let (_directory, path, worker, target) = opened(&bytes);
    let specs = inspect(&worker, &target);
    assert_eq!(specs[0].kind, "value");
    let text = "x'); DELETE FROM Items; --";
    let input = ParameterInput {
        name: "Prompt".into(),
        kind: "text".into(),
        text: text.into(),
        is_null: false,
    };
    let Response::Data {
        rows,
        source_revision,
        ..
    } = request(
        &worker,
        Command::RunSavedQuery {
            target: target.clone(),
            parameters: vec![input.clone()],
        },
    )
    .unwrap()
    else {
        panic!("expected literal parameter result");
    };
    assert_eq!(source_revision, None);
    assert_eq!(rows[0][0].text, text);
    let Response::Data { rows, .. } = request(
        &worker,
        Command::RunSavedQuery {
            target: target.clone(),
            parameters: vec![ParameterInput {
                is_null: true,
                ..input.clone()
            }],
        },
    )
    .unwrap() else {
        panic!("expected explicit NULL");
    };
    assert!(matches!(
        rows[0][0].kind,
        crate::CellKind::Scalar { is_null: true, .. }
    ));
    assert!(matches!(
        request(
            &worker,
            Command::RunSavedQuery {
                target,
                parameters: vec![ParameterInput {
                    kind: String::new(),
                    is_null: true,
                    ..input
                }],
            }
        ),
        Err(Error::ParameterInput)
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn stale_saved_targets_and_unsupported_or_external_definitions_never_execute() {
    let bytes = database_with_query_flags(&[column("1")], 0);
    let (_directory, path, worker, target) = opened(&bytes);
    for invalid in [
        SavedQueryTarget {
            object_id: target.object_id ^ 1,
            ..target.clone()
        },
        SavedQueryTarget {
            name: "saved".into(),
            ..target.clone()
        },
        SavedQueryTarget {
            document_revision: target.document_revision + 1,
            ..target.clone()
        },
    ] {
        assert!(matches!(
            request(&worker, Command::InspectSavedQuery { target: invalid }),
            Err(Error::StaleSavedQuery)
        ));
    }
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::RunSavedQuery {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::StaleSavedQuery)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    let unsupported = database_with_query_flags(&[column("1")], 0x20);
    let (_directory, path, worker, target) = opened(&unsupported);
    assert!(matches!(
        request(&worker, Command::InspectSavedQuery { target }),
        Err(Error::Saved(_))
    ));
    assert_eq!(std::fs::read(path).unwrap(), unsupported);

    let external = database_with_query_flags(
        &[
            Fragment {
                flag: Some(2),
                name1: Some("NEVER_OPEN.accdb"),
                ..row(4)
            },
            Fragment {
                name1: Some("Items"),
                ..row(5)
            },
            column("Id"),
        ],
        0,
    );
    let (_directory, path, worker, target) = opened(&external);
    inspect(&worker, &target);
    assert!(
        request(
            &worker,
            Command::RunSavedQuery {
                target,
                parameters: Vec::new()
            }
        )
        .is_err()
    );
    assert_eq!(std::fs::read(path).unwrap(), external);
}
