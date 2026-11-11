//! Scratch parameter prompts bind immutable SQL to the actual open document.
use super::{open, request};
use crate::{
    Cell, CellKind, Command, Error, InspectedQuery, ParameterInput, ParameterSpec, QueryOperation,
    Response, Worker,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tablecraft_core::values::ExtendedDateTime;
use tablecraft_display::{DateRepresentation, ScalarPresentation};
use tablecraft_expressions::{
    EvalError,
    functions::dates::clock::{Clock, ClockReading},
};
use tablecraft_testkit::fixtures::access::mutation_fixture;

#[derive(Default)]
struct CountingClock(AtomicUsize);

impl Clock for CountingClock {
    fn read(&self) -> Result<ClockReading, EvalError> {
        self.0.fetch_add(1, Ordering::AcqRel);
        ClockReading::new(ExtendedDateTime::new(2026, 11, 7, 13, 14, 15, 0).unwrap())
    }
}

pub(super) fn inspect(
    worker: &Worker,
    document_revision: u64,
    sql: &str,
) -> (Arc<InspectedQuery>, Vec<ParameterSpec>) {
    let Response::QueryParameters { target, parameters } = request(
        worker,
        Command::InspectQuery {
            document_revision,
            sql: sql.to_owned(),
        },
    )
    .unwrap() else {
        panic!("expected inert scratch parameter declarations");
    };
    assert_eq!(target.document_revision(), document_revision);
    assert_eq!(target.sql(), sql);
    (target, parameters)
}

fn input(name: &str, kind: &str, text: &str) -> ParameterInput {
    ParameterInput {
        name: name.to_owned(),
        kind: kind.to_owned(),
        text: text.to_owned(),
        is_null: false,
    }
}

fn rows(response: Response) -> Vec<Vec<Cell>> {
    let Response::Data {
        rows,
        has_more,
        source_revision,
        ..
    } = response
    else {
        panic!("expected complete scratch query result");
    };
    assert!(!has_more);
    assert!(source_revision.is_none());
    rows
}

fn opened(worker: &Worker, path: &std::path::Path) -> u64 {
    let Response::Opened {
        document_revision, ..
    } = request(worker, open(path, "")).unwrap()
    else {
        panic!("expected native document");
    };
    document_revision
}

#[test]
fn inspection_retains_exact_sql_without_scanning_records_or_consulting_clock() {
    let mut original = mutation_fixture::database();
    // Existing raw fixture's sole user DATA page is unreadable; opening and
    // parameter declaration inspection must remain independent of record scans.
    original[12 * 4096] = 0;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unreadable.mdb");
    std::fs::write(&path, &original).unwrap();
    let clock = Arc::new(CountingClock::default());
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    let revision = opened(&worker, &path);
    let mut sql =
        "PARAMETERS [Minimum?] LONG; SELECT [Id], Now() FROM [Items] WHERE [Id] >= [Minimum?]"
            .to_owned();
    let (target, parameters) = inspect(&worker, revision, &sql);
    sql.clear();
    assert!(target.sql().starts_with("PARAMETERS [Minimum?] LONG;"));
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name, "Minimum?");
    assert_eq!(parameters[0].kind, "int32");
    assert_eq!(parameters[0].label, "LONG");
    assert!(parameters[0].nullable);
    assert_eq!(clock.0.load(Ordering::Acquire), 0);
    assert!(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target,
                parameters: vec![input("Minimum?", "int32", "1")],
            }
        )
        .is_err()
    );
    assert_eq!(clock.0.load(Ordering::Acquire), 0);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn exact_declared_constraints_reject_inputs_before_context_and_allow_typed_correction() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("typed.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let clock = Arc::new(CountingClock::default());
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    let revision = opened(&worker, &path);
    let (target, specs) = inspect(
        &worker,
        revision,
        "PARAMETERS [Text] TEXT(2), [Amount] DECIMAL(5,2), [Flag] BIT; SELECT [Text], [Amount], [Flag], Now()",
    );
    assert_eq!(
        specs
            .iter()
            .map(|spec| spec.kind.as_str())
            .collect::<Vec<_>>(),
        ["text", "decimal", "boolean"]
    );
    let valid = vec![
        input("Text", "text", "😀"),
        input("Amount", "decimal", "12.34"),
        input("Flag", "boolean", "true"),
    ];
    let mut invalid_sets = vec![("missing declarations", Vec::new())];
    // The shared declared Text contract counts Unicode scalar values.
    for (case, replacement) in [
        ("text length", input("Text", "text", "😀ab")),
        ("decimal scale", input("Amount", "decimal", "12.345")),
        ("wrong literal kind", input("Flag", "text", "true")),
    ] {
        let mut values = valid.clone();
        let position = values
            .iter()
            .position(|value| value.name == replacement.name)
            .unwrap();
        values[position] = replacement;
        invalid_sets.push((case, values));
    }
    let mut duplicate = valid.clone();
    duplicate[1] = valid[0].clone();
    invalid_sets.push(("duplicate declaration", duplicate));
    for (case, parameters) in invalid_sets {
        let actual = request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target: target.clone(),
                parameters,
            },
        )
        .err();
        assert!(
            matches!(&actual, Some(Error::ParameterInput)),
            "{case} must refuse the declared input: {actual:?}"
        );
        assert_eq!(clock.0.load(Ordering::Acquire), 0, "{case}");
        assert!(worker.poll().is_none());
    }
    let mut malformed = valid.clone();
    malformed[2].text = "not a Boolean literal".to_owned();
    assert!(matches!(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target: target.clone(),
                parameters: malformed,
            }
        ),
        Err(Error::InvalidScalarInput)
    ));
    assert_eq!(clock.0.load(Ordering::Acquire), 0);
    let result = rows(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target: target.clone(),
                parameters: valid,
            },
        )
        .unwrap(),
    );
    assert_eq!(result[0][0].text, "😀");
    assert_eq!(result[0][1].text, "12.34");
    assert!(matches!(
        result[0][2].kind,
        CellKind::Scalar {
            boolean: Some(true),
            ..
        }
    ));
    assert!(matches!(
        result[0][3].presentation,
        Some(ScalarPresentation::DateTime {
            representation: DateRepresentation::LegacyBits(0x40e6_9fb1_a666_6666),
            ..
        })
    ));
    assert_eq!(clock.0.load(Ordering::Acquire), 1);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn value_parameters_keep_literal_text_and_explicit_null_separate_from_sql() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("literal.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let revision = opened(&worker, &path);
    let (target, specs) = inspect(
        &worker,
        revision,
        "PARAMETERS [Prompt] VALUE; SELECT [Prompt]",
    );
    assert_eq!(specs[0].kind, "value");
    let literal = "x'); DELETE FROM Items; --";
    let value = input("Prompt", "text", literal);
    let result = rows(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target: target.clone(),
                parameters: vec![value.clone()],
            },
        )
        .unwrap(),
    );
    assert_eq!(result[0][0].text, literal);
    let result = rows(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target: target.clone(),
                parameters: vec![ParameterInput {
                    is_null: true,
                    ..value.clone()
                }],
            },
        )
        .unwrap(),
    );
    assert!(matches!(
        result[0][0].kind,
        CellKind::Scalar { is_null: true, .. }
    ));
    assert!(matches!(
        request(
            &worker,
            Command::RunQuery {
                options: Default::default(),
                target,
                parameters: vec![ParameterInput {
                    kind: String::new(),
                    is_null: true,
                    ..value
                }]
            }
        ),
        Err(Error::ParameterInput)
    ));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn query_witness_refuses_reopen_and_other_worker_even_with_matching_revision() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("identity.mdb");
    std::fs::write(&path, mutation_fixture::database()).unwrap();
    let clock = Arc::new(CountingClock::default());
    let first = Worker::with_query_clock(|| {}, clock.clone());
    let second = Worker::with_query_clock(|| {}, clock.clone());
    let revision = opened(&first, &path);
    assert_eq!(opened(&second, &path), revision);
    let (target, _) = inspect(
        &first,
        revision,
        "PARAMETERS [Value] LONG; SELECT [Value], Now()",
    );
    let parameters = vec![input("Value", "int32", "7")];
    assert!(matches!(
        request(
            &second,
            Command::RunQuery {
                options: Default::default(),
                target: target.clone(),
                parameters: parameters.clone()
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert_ne!(opened(&first, &path), revision);
    assert!(matches!(
        request(
            &first,
            Command::RunQuery {
                options: Default::default(),
                target,
                parameters
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert!(matches!(
        request(
            &first,
            Command::InspectQuery {
                document_revision: revision,
                sql: "SELECT Now()".to_owned()
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert_eq!(clock.0.load(Ordering::Acquire), 0);
}

#[test]
fn operation_classification_uses_actual_syntax_without_scans_clock_or_mutation() {
    let mut original = mutation_fixture::database();
    original[12 * 4096] = 0;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("classification.mdb");
    std::fs::write(&path, &original).unwrap();
    let clock = Arc::new(CountingClock::default());
    let worker = Worker::with_query_clock(|| {}, clock.clone());
    let revision = opened(&worker, &path);

    for (sql, operation) in [
        ("SELECT Now() FROM Items", QueryOperation::Read),
        (
            "SELECT 'UPDATE Items SET Score=77' AS Note",
            QueryOperation::Read,
        ),
        ("SELECT 1 UNION ALL SELECT 2", QueryOperation::Read),
        (
            "TRANSFORM Count(*) SELECT Id FROM Items GROUP BY Id PIVOT Score IN (10,20,30)",
            QueryOperation::Read,
        ),
        (
            "UPDATE Items SET Score=77 WHERE Id=1",
            QueryOperation::Update,
        ),
        (
            "INSERT INTO Items (Id,Score) VALUES (4,40)",
            QueryOperation::Insert,
        ),
        ("DELETE FROM Items WHERE Id=1", QueryOperation::Delete),
        ("CREATE TABLE NewTable (Id LONG)", QueryOperation::Schema),
        ("DROP TABLE Items", QueryOperation::Schema),
        (
            "SELECT Id INTO Copied FROM Items",
            QueryOperation::Unsupported,
        ),
    ] {
        let (target, parameters) = inspect(&worker, revision, sql);
        assert_eq!(target.operation(), operation, "{sql}");
        assert_eq!(target.sql(), sql);
        assert_eq!(target.document_revision(), revision);
        assert!(parameters.is_empty());
        assert_eq!(clock.0.load(Ordering::Acquire), 0);
    }
    assert_eq!(std::fs::read(path).unwrap(), original);
}
