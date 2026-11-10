//! Captured read allowances are independent, bounded and honored by real workers.
use super::{open, request, row_texts};
use crate::{
    Command, Error, QueryPoolLimits, QueryRunOptions, QueryRunOptionsError, QueryRunPool, Response,
    Worker,
};
use tablecraft_core::limits::Resource;
use tablecraft_query::{QueryError, SourceFailure};
use tablecraft_testkit::fixtures::access::{mutation_fixture, saved_queries};

fn configured(
    native: QueryPoolLimits,
    query: QueryPoolLimits,
    comparison: QueryPoolLimits,
    presentation: QueryPoolLimits,
) -> QueryRunOptions {
    QueryRunOptions::new(
        QueryRunOptions::default().snapshot(),
        native,
        query,
        comparison,
        presentation,
    )
    .unwrap()
}

fn execute(worker: &Worker, options: QueryRunOptions, sql: &str) -> Result<Response, Error> {
    request(
        worker,
        Command::Query {
            sql: sql.into(),
            options,
        },
    )
}

#[test]
fn defaults_preserve_representable_pools_and_invalid_configuration_is_rejected() {
    let defaults = QueryRunOptions::default();
    assert_eq!(defaults.snapshot().maximum_rows(), u32::MAX);
    assert_eq!(defaults.snapshot().batch_rows(), 256);
    assert_eq!(defaults.native().bytes(), u64::MAX);
    assert_eq!(defaults.native().rows(), u32::MAX as u64);
    assert_eq!(defaults.native().steps(), u64::MAX);
    assert_eq!(defaults.query().bytes(), u64::MAX);
    assert_eq!(defaults.query().rows(), u64::MAX);
    assert_eq!(defaults.query().steps(), u64::MAX);
    assert_eq!(defaults.comparison().bytes(), u64::MAX);
    assert_eq!(defaults.comparison().rows(), 0);
    assert_eq!(defaults.comparison().steps(), u64::MAX);
    assert_eq!(defaults.presentation().bytes(), u64::MAX);
    assert_eq!(defaults.presentation().rows(), 0);
    assert_eq!(defaults.presentation().steps(), u64::MAX);

    for limits in [(0, 0, 1), (1, 0, 0)] {
        assert_eq!(
            QueryPoolLimits::new(limits.0, limits.1, limits.2),
            Err(QueryRunOptionsError::PoolBounds)
        );
    }

    let non_row = QueryPoolLimits::new(1, 0, 1).unwrap();
    assert_eq!(
        QueryRunOptions::new(
            defaults.snapshot(),
            non_row,
            defaults.query(),
            defaults.comparison(),
            defaults.presentation(),
        ),
        Err(QueryRunOptionsError::RowAllowance)
    );
    assert_eq!(
        QueryRunOptions::new(
            defaults.snapshot(),
            defaults.native(),
            defaults.query(),
            defaults.query(),
            defaults.presentation(),
        ),
        Err(QueryRunOptionsError::NonRowPool)
    );
    assert_eq!(
        QueryRunOptions::with_limits(
            0,
            256,
            defaults.native(),
            defaults.query(),
            defaults.comparison(),
            defaults.presentation(),
        ),
        Err(QueryRunOptionsError::SnapshotBounds)
    );
    let explicit = QueryPoolLimits::new(5 * 1024 * 1024 * 1024, 11_000_000, 2_000_000_000).unwrap();
    let configured = QueryRunOptions::new(
        defaults.snapshot(),
        explicit,
        explicit,
        defaults.comparison(),
        defaults.presentation(),
    )
    .unwrap();
    assert_eq!(configured.native(), explicit);
    assert_eq!(configured.query(), explicit);
    let representable = QueryPoolLimits::new(u64::MAX, u64::MAX, u64::MAX).unwrap();
    assert_eq!(representable.bytes(), u64::MAX);
    assert_eq!(representable.rows(), u64::MAX);
    assert_eq!(representable.steps(), u64::MAX);
}

#[test]
fn snapshot_and_cumulative_pools_fail_without_output_or_changing_the_next_run() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("limits.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    request(&worker, open(&path, "")).unwrap();
    let defaults = QueryRunOptions::default();
    let sql = "SELECT Id FROM Items ORDER BY Id";

    let limited = QueryRunOptions::with_limits(
        2,
        1,
        defaults.native(),
        defaults.query(),
        defaults.comparison(),
        defaults.presentation(),
    )
    .unwrap();
    assert!(matches!(
        execute(&worker, limited, sql),
        Err(Error::Query(QueryError::SourceFailure(
            SourceFailure::StructureLimit
        )))
    ));

    let native = configured(
        QueryPoolLimits::new(1, 100_000, 5_000_000).unwrap(),
        defaults.query(),
        defaults.comparison(),
        defaults.presentation(),
    );
    assert!(matches!(
        execute(&worker, native, sql),
        Err(Error::ReadQueryLimit {
            pool: QueryRunPool::Native,
            source: tablecraft_core::limits::LimitError {
                resource: Resource::Bytes,
                ..
            },
        })
    ));

    let query = configured(
        defaults.native(),
        QueryPoolLimits::new(1, 10_000, 1_000_000).unwrap(),
        defaults.comparison(),
        defaults.presentation(),
    );
    assert!(matches!(
        execute(&worker, query, sql),
        Err(Error::Query(QueryError::Limit(_)))
    ));

    let presentation = configured(
        defaults.native(),
        defaults.query(),
        defaults.comparison(),
        QueryPoolLimits::new(1, 0, 100_000).unwrap(),
    );
    assert!(matches!(
        execute(&worker, presentation, sql),
        Err(Error::ReadQueryLimit {
            pool: QueryRunPool::Presentation,
            ..
        })
    ));

    let Response::Data { rows, has_more, .. } = execute(&worker, defaults, sql).unwrap() else {
        panic!("expected complete default result after rejected limited runs");
    };
    assert_eq!(row_texts(&rows), [["1"], ["2"], ["3"]]);
    assert!(!has_more);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn inspected_and_saved_runs_use_supplied_options_without_weakening_target_identity() {
    let fragments = [saved_queries::Fragment {
        object: Some(saved_queries::QUERY_ID),
        attribute: Some(6),
        expression: Some("42"),
        order: Some(&[0, 0, 0, 1]),
        ..Default::default()
    }];
    let bytes = saved_queries::database_with_query_flags(&fragments, 0);
    let (_directory, path, worker, saved) = super::saved::opened(&bytes);
    let (target, _) =
        super::query_parameters::inspect(&worker, saved.document_revision, "SELECT 42 AS Answer");
    let defaults = QueryRunOptions::default();
    let tiny = configured(
        defaults.native(),
        defaults.query(),
        defaults.comparison(),
        QueryPoolLimits::new(1, 0, 100_000).unwrap(),
    );

    for command in [
        Command::RunQuery {
            target: target.clone(),
            parameters: Vec::new(),
            options: tiny,
        },
        Command::RunSavedQuery {
            target: saved.clone(),
            parameters: Vec::new(),
            options: tiny,
        },
    ] {
        assert!(matches!(
            request(&worker, command),
            Err(Error::ReadQueryLimit {
                pool: QueryRunPool::Presentation,
                ..
            })
        ));
    }

    for command in [
        Command::RunQuery {
            target,
            parameters: Vec::new(),
            options: defaults,
        },
        Command::RunSavedQuery {
            target: saved.clone(),
            parameters: Vec::new(),
            options: defaults,
        },
    ] {
        let Response::Data { rows, .. } = request(&worker, command).unwrap() else {
            panic!("expected complete explicitly selected query result");
        };
        assert_eq!(row_texts(&rows), [["42"]]);
    }

    let mut stale = saved;
    stale.document_revision += 1;
    assert!(matches!(
        request(
            &worker,
            Command::RunSavedQuery {
                target: stale,
                parameters: Vec::new(),
                options: tiny,
            }
        ),
        Err(Error::StaleSavedQuery)
    ));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn comparison_allowance_is_independent_and_inspection_does_not_consume_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("comparison.mdb");
    let mut original = mutation_fixture::database();
    super::text_policy::database_order(&mut original, [9, 4, 0, 0]);
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected native document");
    };
    let (target, _) =
        super::query_parameters::inspect(&worker, document_revision, "SELECT 'a'='A' AS Same");
    let defaults = QueryRunOptions::default();
    let tiny = configured(
        defaults.native(),
        defaults.query(),
        QueryPoolLimits::new(1, 0, 1_000_000).unwrap(),
        defaults.presentation(),
    );
    assert!(matches!(
        request(
            &worker,
            Command::RunQuery {
                target: target.clone(),
                parameters: Vec::new(),
                options: tiny,
            }
        ),
        Err(Error::Query(QueryError::Limit(
            tablecraft_core::limits::LimitError {
                resource: Resource::Bytes,
                remaining: 1,
            }
        )))
    ));

    let Response::Data { rows, .. } = request(
        &worker,
        Command::RunQuery {
            target,
            parameters: Vec::new(),
            options: defaults,
        },
    )
    .unwrap() else {
        panic!("expected successful independently replenished read run");
    };
    assert_eq!(row_texts(&rows), [["true"]]);
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn source_failure_adaptation_labels_only_actual_native_resource_exhaustion() {
    use crate::query::options::query_error;
    use tablecraft_core::limits::LimitError;

    let exhausted = LimitError {
        resource: Resource::Rows,
        remaining: 2,
    };
    assert!(matches!(
        query_error(QueryError::SourceFailure(SourceFailure::Limit(exhausted))),
        Error::ReadQueryLimit {
            pool: QueryRunPool::Native,
            source,
        } if source == exhausted
    ));
    assert!(matches!(
        query_error(QueryError::Limit(exhausted)),
        Error::Query(QueryError::Limit(source)) if source == exhausted
    ));
    for failure in [
        SourceFailure::Io(std::io::ErrorKind::PermissionDenied),
        SourceFailure::Truncated,
        SourceFailure::InvalidData,
        SourceFailure::Busy,
        SourceFailure::InvalidRequest,
        SourceFailure::CursorState,
        SourceFailure::Constraint,
        SourceFailure::StructureLimit,
        SourceFailure::Allocation,
    ] {
        assert!(matches!(
            query_error(QueryError::SourceFailure(failure)),
            Error::Query(QueryError::SourceFailure(actual)) if actual == failure
        ));
    }
}

#[test]
fn actual_native_snapshot_row_work_reports_its_source_pool_and_allows_explicit_new_run() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native-rows.mdb");
    let original = mutation_fixture::database();
    std::fs::write(&path, &original).unwrap();
    let worker = Worker::new(|| {});
    request(&worker, open(&path, "")).unwrap();
    let defaults = QueryRunOptions::default();
    let tiny = configured(
        QueryPoolLimits::new(defaults.native().bytes(), 4, defaults.native().steps()).unwrap(),
        defaults.query(),
        defaults.comparison(),
        defaults.presentation(),
    );
    assert!(matches!(
        execute(&worker, tiny, "SELECT Id FROM Items"),
        Err(Error::ReadQueryLimit {
            pool: QueryRunPool::Native,
            source: tablecraft_core::limits::LimitError {
                resource: Resource::Rows,
                ..
            },
        })
    ));
    let Response::Data { rows, .. } = execute(&worker, defaults, "SELECT Id FROM Items").unwrap()
    else {
        panic!("an explicit fresh run uses its own native budget");
    };
    assert_eq!(row_texts(&rows), [["1"], ["2"], ["3"]]);
    assert_eq!(std::fs::read(path).unwrap(), original);
}
