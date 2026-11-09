//! Classified host failures cross metadata, discovery and publication unchanged.
use std::{cell::Cell, io::ErrorKind};
use tablecraft_core::{
    limits::{LimitError, Resource},
    values::Value,
};
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, SourceFailure, StreamError,
    execute_with_schema_discovery, prepare,
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

#[derive(Clone, Copy)]
enum FailurePoint {
    Metadata,
    BeforeRows,
    AfterRow,
    IgnoreVisitorFailure,
}

struct FailingSource {
    columns: [Column; 1],
    pivot: Query,
    failure: QueryError,
    point: FailurePoint,
    scans: Cell<usize>,
}

impl FailingSource {
    fn new(failure: QueryError, point: FailurePoint) -> Self {
        Self {
            columns: [Column::new("K".into(), ScalarType::Int32, false).unwrap()],
            pivot: parse_query(
                "TRANSFORM Count(*) SELECT K FROM T GROUP BY K PIVOT K",
                Default::default(),
            )
            .unwrap()
            .into_query(),
            failure,
            point,
            scans: Cell::new(0),
        }
    }
}

impl Source for FailingSource {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Pivoted").then_some(&self.pivot))
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        if matches!(self.point, FailurePoint::Metadata) {
            return Err(self.failure);
        }
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        if matches!(self.point, FailurePoint::AfterRow) {
            visitor(&[Value::Int32(1)])?;
        }
        Err(self.failure)
    }

    fn scan_projected_progressive(
        &self,
        _: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        let row = [Value::Int32(1)];
        if matches!(self.point, FailurePoint::AfterRow) {
            visitor(ProjectedRow::select(&row, indices)?)?;
        } else if matches!(self.point, FailurePoint::IgnoreVisitorFailure) {
            // Deliberately broken provider: the executor must preserve the first
            // consumer failure even when the provider later reports another.
            let _ = visitor(ProjectedRow::select(&row, indices)?);
        }
        Err(self.failure)
    }
}

fn classified_failures() -> [SourceFailure; 10] {
    [
        SourceFailure::Io(ErrorKind::PermissionDenied),
        SourceFailure::Truncated,
        SourceFailure::InvalidData,
        SourceFailure::Busy,
        SourceFailure::InvalidRequest,
        SourceFailure::CursorState,
        SourceFailure::Constraint,
        SourceFailure::Limit(LimitError {
            resource: Resource::Bytes,
            remaining: 17,
        }),
        SourceFailure::StructureLimit,
        SourceFailure::Allocation,
    ]
}

#[test]
fn metadata_and_buffered_reads_preserve_every_classified_failure() {
    let parsed = parse_query("SELECT K FROM T", Default::default()).unwrap();
    for failure in classified_failures() {
        let expected = QueryError::SourceFailure(failure);
        let metadata = FailingSource::new(expected, FailurePoint::Metadata);
        assert_eq!(prepare(parsed.query(), &metadata).err(), Some(expected));
        assert_eq!(metadata.scans.get(), 0);

        for point in [FailurePoint::BeforeRows, FailurePoint::AfterRow] {
            let source = FailingSource::new(expected, point);
            let plan = prepare(parsed.query(), &source).unwrap();
            assert_eq!(source.scans.get(), 0);
            assert_eq!(
                plan.execute(&source, &[], ExecutionOptions::default())
                    .err(),
                Some(expected)
            );
            assert_eq!(source.scans.get(), 1);
        }
    }
}

#[test]
fn dynamic_named_schema_discovery_does_not_replace_source_failures_or_cache_prefixes() {
    let parsed = parse_query("SELECT * FROM Pivoted", Default::default()).unwrap();
    for failure in classified_failures() {
        let expected = QueryError::SourceFailure(failure);
        let source = FailingSource::new(expected, FailurePoint::AfterRow);
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Unsupported)
        );
        assert_eq!(source.scans.get(), 0);

        for attempt in 1..=2 {
            assert_eq!(
                execute_with_schema_discovery(
                    parsed.query(),
                    &source,
                    &[],
                    ExecutionOptions::default(),
                )
                .err(),
                Some(expected)
            );
            assert_eq!(source.scans.get(), attempt);
        }
    }
}

#[test]
fn progressive_late_failure_retains_published_rows_and_first_visitor_error() {
    let parsed = parse_query("SELECT K FROM T", Default::default()).unwrap();
    let expected = QueryError::SourceFailure(SourceFailure::Truncated);
    let source = FailingSource::new(expected, FailurePoint::AfterRow);
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut published = Vec::new();
    let error = plan
        .execute_progressive(&source, &[], ExecutionOptions::default(), &mut |row| {
            published.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap_err();
    assert!(matches!(error, StreamError::Query(actual) if actual == expected));
    assert_eq!(published, [vec![Value::Int32(1)]]);

    let source = FailingSource::new(expected, FailurePoint::IgnoreVisitorFailure);
    let parsed = parse_query("SELECT 1/(K-1) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut published = 0;
    let error = plan
        .execute_progressive(&source, &[], ExecutionOptions::default(), &mut |_| {
            published += 1;
            Ok::<_, ()>(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
    ));
    assert_eq!(published, 0);
}

#[test]
fn diagnostics_are_payload_free_and_cancellation_and_legacy_source_errors_stay_distinct() {
    assert_eq!(
        QueryError::from(SourceFailure::Io(ErrorKind::PermissionDenied)).to_string(),
        "query source failed: I/O failed (PermissionDenied)"
    );
    assert_eq!(
        QueryError::from(SourceFailure::Limit(LimitError {
            resource: Resource::Bytes,
            remaining: 17,
        }))
        .to_string(),
        "query source failed: Bytes resource limit exceeded (17 remaining)"
    );
    assert_eq!(QueryError::Source.to_string(), "query source failed");

    let parsed = parse_query("SELECT K FROM T", Default::default()).unwrap();
    for expected in [QueryError::Source, QueryError::Cancelled] {
        let source = FailingSource::new(expected, FailurePoint::AfterRow);
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .err(),
            Some(expected)
        );
    }
}
