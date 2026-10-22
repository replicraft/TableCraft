use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, prepare, source::MemorySource,
};
use tablecraft_schema::Column;
use tablecraft_sql::parse_query;
use tablecraft_sql::query::Query;

fn execute(sql: &str) -> Vec<Vec<ExpressionValue>> {
    let source = MemorySource::default();
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), &source)
        .unwrap()
        .execute_expression(&source, &[], Default::default())
        .unwrap()
        .rows()
        .to_vec()
}

#[test]
fn result_cells_preserve_error_codes_and_scalar_callers_reject_them() {
    assert_eq!(
        execute("SELECT CVErr(0),CVErr(2001),Null,2001,'#ERROR(2001)',IIf(True,CVErr(65535),0)"),
        [vec![
            ExpressionValue::Error(0),
            ExpressionValue::Error(2001),
            ExpressionValue::Value(Value::Null),
            ExpressionValue::Value(Value::Int64(2001)),
            ExpressionValue::Value(Value::Text("#ERROR(2001)".into())),
            ExpressionValue::Error(65535),
        ]]
    );
    let source = MemorySource::default();
    let query = parse_query("SELECT CVErr(2001)", Default::default()).unwrap();
    let prepared = prepare(query.query(), &source).unwrap();
    assert_eq!(
        prepared.execute(&source, &[], Default::default()).err(),
        Some(EvalError::NonScalarExpression.into())
    );
    assert_eq!(
        prepared
            .execute_expression(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![ExpressionValue::Error(2001)]]
    );
    assert!(Value::try_from(ExpressionValue::Error(2001)).is_err());
}

#[test]
fn derived_union_join_and_correlated_rows_retain_expression_data() {
    for sql in [
        "SELECT Problem FROM (SELECT CVErr(2001) AS Problem) AS Q",
        "SELECT CVErr(2001) UNION ALL SELECT CVErr(2002)",
        "SELECT Q.Problem FROM (SELECT CVErr(2001) AS Problem) AS Q INNER JOIN (SELECT 1 AS ID) AS R ON IsError(Q.Problem)",
        "SELECT (SELECT Q.Problem) FROM (SELECT CVErr(2001) AS Problem) AS Q",
        "SELECT CVErr(2001) AS Problem ORDER BY 1+0",
    ] {
        let rows = execute(sql);
        assert_eq!(rows[0], [ExpressionValue::Error(2001)], "{sql}");
        if sql.contains("UNION") {
            assert_eq!(rows[1], [ExpressionValue::Error(2002)]);
        }
    }
    assert_eq!(
        execute(
            "SELECT IsError(Problem),TypeName(Problem),CLng(Problem),Problem IS NULL FROM (SELECT CVErr(2001) AS Problem) AS Q"
        ),
        [vec![
            ExpressionValue::Value(Value::Boolean(true)),
            ExpressionValue::Value(Value::Text("Error".into())),
            ExpressionValue::Value(Value::Int32(2001)),
            ExpressionValue::Value(Value::Boolean(false)),
        ]]
    );
    assert_eq!(
        execute(
            "SELECT Q.Problem FROM (SELECT CVErr(2001) AS Problem) AS Q WHERE IsError(Q.Problem)"
        ),
        [vec![ExpressionValue::Error(2001)]]
    );
}

struct NamedError {
    query: Query,
}

impl Source for NamedError {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(name.eq_ignore_ascii_case("Problems").then_some(&self.query))
    }

    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Err(QueryError::UnknownName)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("a named inline query must not scan native source rows")
    }
}

#[test]
fn named_queries_and_crosstab_publication_share_expression_rows() {
    let source = NamedError {
        query: parse_query("SELECT CVErr(2001) AS Problem", Default::default())
            .unwrap()
            .into_query(),
    };
    let query = parse_query("SELECT Problem FROM Problems", Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute_expression(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![ExpressionValue::Error(2001)]]
    );
    assert_eq!(
        execute(
            "TRANSFORM CVErr(Sum(N)) SELECT N FROM (SELECT 1 AS N,1 AS P) AS Q GROUP BY N PIVOT P IN (1,2)"
        ),
        [vec![
            ExpressionValue::Value(Value::Int64(1)),
            ExpressionValue::Error(1),
            ExpressionValue::Value(Value::Null),
        ]]
    );
}

#[test]
fn operational_failures_and_unimplemented_error_coercions_remain_explicit() {
    let source = MemorySource::default();
    for (sql, expected) in [
        ("SELECT CVErr(2001),1/0", EvalError::DivisionByZero),
        (
            "SELECT IIf(True,CVErr(2001),1/0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT CVErr(-1)", EvalError::Domain),
        ("SELECT IsError(1/0)", EvalError::DivisionByZero),
        ("SELECT CVErr(2001)+1", EvalError::TypeMismatch),
        (
            "SELECT CVErr(2001) AS Problem ORDER BY Problem",
            EvalError::NonScalarExpression,
        ),
        (
            "SELECT SUM(Problem) FROM (SELECT CVErr(2001) AS Problem) AS Q",
            EvalError::NonScalarExpression,
        ),
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source)
                .unwrap()
                .execute_expression(&source, &[], Default::default())
                .err(),
            Some(expected.into()),
            "{sql}"
        );
    }
}

#[test]
fn every_observed_checkpoint_cancels_and_result_buffers_share_quotas() {
    let source = MemorySource::default();
    let query = parse_query(
        "SELECT Problem FROM (SELECT CVErr(2001) AS Problem UNION ALL SELECT CVErr(2002)) AS Q WHERE IsError(Problem)",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(query.query(), &source).unwrap();
    let checks = Cell::new(0);
    let observed = || {
        checks.set(checks.get() + 1);
        false
    };
    prepared
        .execute_expression(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&observed),
                ..Default::default()
            },
        )
        .unwrap();
    for stop in 1..=checks.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(matches!(
            prepared.execute_expression(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancelled),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            rows: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(
            prepared
                .execute_expression(
                    &source,
                    &[],
                    ExecutionOptions {
                        limits,
                        ..Default::default()
                    }
                )
                .is_err()
        );
    }
}
