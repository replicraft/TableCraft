use std::cell::Cell;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn explicit_cstr_consumes_error_cells_in_derived_correlated_group_and_domain_queries() {
    let source = source();
    for (sql, expected) in [
        (
            "SELECT CStr(E) FROM (SELECT CVErr(2001) AS E UNION ALL SELECT CVErr(2002)) AS Q",
            vec![
                vec![Value::Text("Error 2001".into())],
                vec![Value::Text("Error 2002".into())],
            ],
        ),
        (
            "SELECT (SELECT CStr(CVErr(T.ID))) FROM T ORDER BY ID",
            vec![
                vec![Value::Text("Error 1".into())],
                vec![Value::Text("Error 2".into())],
            ],
        ),
        (
            "SELECT CStr(First(CVErr(ID))),CStr(Last(CVErr(ID))) FROM T",
            vec![vec![
                Value::Text("Error 1".into()),
                Value::Text("Error 2".into()),
            ]],
        ),
        (
            "SELECT CStr(DLookup('CVErr(ID)','T')),CStr(DFirst('CVErr(ID)','T')),CStr(DLast('CVErr(ID)','T'))",
            vec![vec![
                Value::Text("Error 1".into()),
                Value::Text("Error 1".into()),
                Value::Text("Error 2".into()),
            ]],
        ),
        (
            "SELECT CStr(Nz(Null)),CStr(IIf(True,CVErr(1),0)),IsError(CStr(CVErr(1))),VarType(CStr(CVErr(1)))",
            vec![vec![
                Value::Text("".into()),
                Value::Text("Error 1".into()),
                Value::Boolean(false),
                Value::Int16(8),
            ]],
        ),
    ] {
        let plan = prepare(&syntax(sql), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            expected,
            "{sql}"
        );
    }
    let query = syntax(
        "TRANSFORM First(CStr(CVErr(ID))) SELECT True AS G FROM T GROUP BY True PIVOT ID IN (1,2)",
    );
    let result = prepare(&query, &source)
        .unwrap()
        .execute(&source, &[], Default::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            Value::Boolean(true),
            Value::Text("Error 1".into()),
            Value::Text("Error 2".into())
        ]]
    );
}

struct Named {
    query: Query,
}
impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Errors").then_some(&self.query))
    }
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        panic!("closed named expression needs no source columns")
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("closed named expression must not scan sources")
    }
}

#[test]
fn metadata_and_named_values_preserve_explicit_result_and_raw_error_boundaries() {
    let source = Named {
        query: syntax("SELECT CVErr(2001) AS Problem"),
    };
    let query = syntax("SELECT CStr(Problem) FROM Errors");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [OutputType::Scalar {
            data_type: TypeDescriptor::new(ScalarType::Text),
            nullable: true
        }]
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Text("Error 2001".into())]]
    );
    let plan = prepare(&syntax("SELECT Problem,CStr(Problem) FROM Errors"), &source).unwrap();
    assert_eq!(
        plan.execute_expression(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![
            ExpressionValue::Error(2001),
            ExpressionValue::Value(Value::Text("Error 2001".into()))
        ]]
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NonScalarExpression))
    );
}

#[test]
fn only_explicit_cstr_admits_error_text_and_eager_failures_still_fail() {
    let source = source();
    for (sql, error) in [
        ("SELECT CStr(Null)", EvalError::InvalidNullConversion),
        (
            "SELECT CStr(IIf(True,CVErr(1),1/0))",
            EvalError::DivisionByZero,
        ),
        ("SELECT CStr(CVErr(65536))", EvalError::Domain),
        ("SELECT CStr(1)", EvalError::NumberFormatRequired),
        ("SELECT CVErr(1) & 'x'", EvalError::NonScalarExpression),
        (
            "SELECT CStr(CVErr(ID)),1/(2-ID) FROM T",
            EvalError::DivisionByZero,
        ),
    ] {
        let plan = prepare(&syntax(sql), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(error)),
            "{sql}"
        );
    }
}

#[test]
fn standard_error_string_conversion_keeps_shared_query_cancellation() {
    let source = source();
    let plan = prepare(
        &syntax("SELECT CStr(CVErr(ID)) FROM T ORDER BY ID"),
        &source,
    )
    .unwrap();
    let calls = Cell::new(0);
    let run = |stop| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    assert!(run(usize::MAX).is_ok());
    let count = calls.get();
    for stop in 1..=count {
        assert!(
            matches!(
                run(stop),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
}
