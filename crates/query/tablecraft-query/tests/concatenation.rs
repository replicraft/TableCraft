use std::cmp::Ordering;
use tablecraft_core::values::{Currency, Double, ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError,
    comparison::Collation,
    functions::{conversions::text::NumberFormat, text::cstr::DateTextFormat},
};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, execute_with_schema_discovery, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}
struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}
fn options() -> ExecutionOptions<'static> {
    ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        date_format: Some(DateTextFormat::Iso),
        collation: Some(&Ordinal),
        ..Default::default()
    }
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("ID".into(), ScalarType::Int64, false).unwrap(),
                Column::new("Money".into(), ScalarType::Currency, true).unwrap(),
            ],
            vec![
                vec![
                    Value::Int64(9007199254740993),
                    Value::Currency(Currency::from_scaled(12345)),
                ],
                vec![Value::Int64(2), Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn concatenation_flows_through_rows_parameters_groups_and_nested_queries() {
    let source = source();
    let query = syntax(
        "PARAMETERS S TEXT; SELECT ID & S AS Label,(SELECT [Money] & S) AS Amount FROM T ORDER BY ID",
    );
    let plan = prepare(&query, &source).unwrap();
    for (format, expected) in [
        (NumberFormat::EnUs, "1.2345!"),
        (NumberFormat::DeDe, "1,2345!"),
    ] {
        let result = plan
            .execute(
                &source,
                &[("S", Value::Text("!".into()))],
                ExecutionOptions {
                    number_format: Some(format),
                    ..options()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Text("2!".into()), Value::Text("!".into())],
                vec![
                    Value::Text("9007199254740993!".into()),
                    Value::Text(expected.into())
                ]
            ]
        );
    }
    let result = prepare(
        &syntax("SELECT Count(*) FROM T GROUP BY [Money] & ':'"),
        &source,
    )
    .unwrap()
    .execute(&source, &[], options())
    .unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int64(1)], vec![Value::Int64(1)]]
    );
    let result = prepare(
        &syntax("SELECT X FROM (SELECT 'n=' & 1 AS X UNION ALL SELECT 'n=' & 2) AS Q ORDER BY X"),
        &source,
    )
    .unwrap()
    .execute(&source, &[], options())
    .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Text("n=1".into())],
            vec![Value::Text("n=2".into())]
        ]
    );
}

#[test]
fn eager_errors_and_context_failures_are_not_hidden_by_null_or_selectors() {
    let source = source();
    for (sql, expected) in [
        ("SELECT Null & (1/0)", EvalError::DivisionByZero),
        (
            "SELECT IIf(False,1 & 'x','ok')",
            EvalError::NumberFormatRequired,
        ),
        (
            "SELECT [Money] & 'x' FROM T",
            EvalError::NumberFormatRequired,
        ),
        ("SELECT #2024-01-02# & 'x'", EvalError::DateFormatRequired),
    ] {
        let plan = prepare(&syntax(sql), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(expected))
        );
    }
    let result = prepare(&syntax("SELECT 'a' & Null,Null & Null,'2'+3"), &source)
        .unwrap()
        .execute(&source, &[], options())
        .unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            Value::Text("a".into()),
            Value::Null,
            Value::Double(Double::from_float(5.0))
        ]]
    );
}

struct NoScan {
    columns: Vec<Column>,
    view: Query,
}
impl Source for NoScan {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "V").then_some(&self.view))
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("metadata must not scan")
    }
}

#[test]
fn concatenation_metadata_handles_two_nulls_and_nullable_operands_without_scans() {
    let source = NoScan {
        columns: vec![Column::new("N".into(), ScalarType::Int32, true).unwrap()],
        view: syntax("SELECT N & 'x' AS X FROM T"),
    };
    let plan = prepare(
        &syntax("SELECT N & 'x',N & Null,Null & Null,True & False FROM T"),
        &source,
    )
    .unwrap();
    let text = |nullable| OutputType::Scalar {
        data_type: TypeDescriptor::new(ScalarType::Text),
        nullable,
    };
    assert_eq!(
        plan.output_types().unwrap(),
        [text(false), text(true), OutputType::Null, text(false)]
    );
    assert_eq!(
        prepare(&syntax("SELECT * FROM V"), &source)
            .unwrap()
            .output_types()
            .unwrap(),
        [text(false)]
    );
}

#[test]
fn contextual_named_pivot_headings_are_discovered_only_during_explicit_execution() {
    struct Views {
        data: MemorySource,
        query: Query,
    }
    impl Source for Views {
        fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
            self.data.columns(name)
        }
        fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
            Ok((name == "Piv").then_some(&self.query))
        }
        fn scan(
            &self,
            name: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            self.data.scan(name, visitor)
        }
    }
    let source = Views {
        data: source(),
        query: syntax(
            "TRANSFORM Count(*) SELECT True AS G FROM T GROUP BY True PIVOT 'h' & 1 IN ('h' & 1)",
        ),
    };
    let query = syntax("SELECT * FROM Piv");
    assert_eq!(
        prepare(&query, &source).err(),
        Some(QueryError::Unsupported)
    );
    let result = execute_with_schema_discovery(&query, &source, &[], options()).unwrap();
    assert_eq!(result.columns(), ["G", "h1"]);
    assert_eq!(
        result.rows(),
        [vec![
            ExpressionValue::Value(Value::Boolean(true)),
            ExpressionValue::Value(Value::Int64(2))
        ]]
    );
}
