use std::cell::Cell;
use tablecraft_core::values::{Decimal, Double, ExpressionValue, Single, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryLimits, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}
fn scalar(kind: ScalarType, nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(kind),
        nullable,
    }
}
fn run(sql: &str) -> tablecraft_query::QueryResult {
    let source = MemorySource::new(vec![]).unwrap();
    prepare(&syntax(sql), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap()
}

#[test]
fn integer_arms_widen_before_distinct_sorting_and_outer_consumers() {
    for sql in [
        "SELECT CByte(255) AS N UNION ALL SELECT CInt(-32768) UNION ALL SELECT CLng(2147483647) UNION ALL SELECT CLngLng(9223372036854775807) ORDER BY N",
        "SELECT CLngLng(9223372036854775807) AS N UNION ALL SELECT CLng(2147483647) UNION ALL SELECT CInt(-32768) UNION ALL SELECT CByte(255) ORDER BY N",
    ] {
        let result = run(sql);
        assert_eq!(result.output_types(), [scalar(ScalarType::Int64, false)]);
        assert_eq!(
            result.rows(),
            &[
                vec![Value::Int64(-32768)],
                vec![Value::Int64(255)],
                vec![Value::Int64(2147483647)],
                vec![Value::Int64(i64::MAX)]
            ]
        );
    }
    assert_eq!(
        run("SELECT CByte(1) AS N UNION SELECT CInt(1) UNION ALL SELECT CLng(1)").rows(),
        &[vec![Value::Int32(1)], vec![Value::Int32(1)]]
    );
    assert_eq!(
        run("SELECT CByte(1) AS N UNION ALL SELECT CInt(1) UNION SELECT CLng(1)").rows(),
        &[vec![Value::Int32(1)]]
    );
    assert_eq!(run("SELECT TypeName(U.N),VarType(U.N) FROM (SELECT CByte(1) AS N UNION ALL SELECT CLng(2)) AS U").rows(), &[vec![Value::Text("Long".into()), Value::Int16(3)], vec![Value::Text("Long".into()), Value::Int16(3)]]);
    assert_eq!(
        run("SELECT Count(*) FROM (SELECT CByte(1) AS N UNION SELECT CLngLng(1)) AS U").rows(),
        &[vec![Value::Int64(1)]]
    );
}

#[test]
fn null_arms_empty_typed_arms_and_parameters_do_not_require_record_inference() {
    let source = MemorySource::new(vec![]).unwrap();
    let parsed =
        syntax("PARAMETERS p LONG; SELECT CByte(1) AS N UNION ALL SELECT p UNION ALL SELECT Null");
    let plan = prepare(&parsed, &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [scalar(ScalarType::Int32, true)]
    );
    for value in [Value::Int32(i32::MIN), Value::Int32(i32::MAX), Value::Null] {
        assert_eq!(
            plan.execute(
                &source,
                &[("p", value.clone())],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows(),
            &[vec![Value::Int32(1)], vec![value], vec![Value::Null]]
        );
    }
    let result = run("SELECT CByte(1) AS N UNION ALL SELECT CLngLng(9) WHERE False");
    assert_eq!(result.rows(), &[vec![Value::Int64(1)]]);
    assert_eq!(result.output_types(), [scalar(ScalarType::Int64, false)]);
}

#[test]
fn single_and_double_arms_widen_exactly_including_signed_zero() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("S".into(), ScalarType::Single, false).unwrap(),
                Column::new("D".into(), ScalarType::Double, false).unwrap(),
            ],
            vec![
                vec![
                    Value::Single(Single::from_float(-0.0)),
                    Value::Double(Double::from_float(1.0)),
                ],
                vec![
                    Value::Single(Single::from_bits(1)),
                    Value::Double(Double::from_float(f64::MAX)),
                ],
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = syntax("SELECT S AS N FROM T UNION ALL SELECT D FROM T");
    let plan = prepare(&parsed, &source).unwrap();
    assert_eq!(
        plan.output_types().unwrap(),
        [scalar(ScalarType::Double, false)]
    );
    let result = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(
        result.rows(),
        &[
            vec![Value::Double(Double::from_float(-0.0))],
            vec![Value::Double(Double::from_float(f64::from(
                f32::from_bits(1)
            )))],
            vec![Value::Double(Double::from_float(1.0))],
            vec![Value::Double(Double::from_float(f64::MAX))],
        ]
    );
}

struct MetadataOnly {
    table: MemorySource,
    named: Query,
    scans: Cell<usize>,
    allowed: Cell<bool>,
}
impl Source for MetadataOnly {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.table.columns(name)
    }
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "V").then_some(&self.named))
    }
    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(self.allowed.get(), "binding must not inspect records");
        self.scans.set(self.scans.get() + 1);
        self.table.scan(name, visitor)
    }
}

#[test]
fn declared_text_decimal_and_named_result_descriptors_are_shared_without_rescaling() {
    let source = MetadataOnly {
        table: MemorySource::new(vec![
            MemoryTable::new(
                "T".into(),
                vec![
                    Column::with_type("A".into(), TypeDescriptor::text(2).unwrap(), false).unwrap(),
                    Column::with_type("B".into(), TypeDescriptor::text(5).unwrap(), true).unwrap(),
                    Column::with_type("X".into(), TypeDescriptor::decimal(4, 2).unwrap(), false)
                        .unwrap(),
                    Column::with_type("Y".into(), TypeDescriptor::decimal(6, 3).unwrap(), false)
                        .unwrap(),
                ],
                vec![vec![
                    Value::Text("é🦀".into()),
                    Value::Text("hello".into()),
                    Value::Decimal(Decimal::new(100, 2).unwrap()),
                    Value::Decimal(Decimal::new(1000, 3).unwrap()),
                ]],
            )
            .unwrap(),
        ])
        .unwrap(),
        named: syntax("SELECT A AS N,X AS D FROM T UNION ALL SELECT B,Y FROM T"),
        scans: Cell::new(0),
        allowed: Cell::new(false),
    };
    let parsed = syntax("SELECT * FROM V");
    let plan = prepare(&parsed, &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.output_types().unwrap(),
        [
            OutputType::Scalar {
                data_type: TypeDescriptor::text(5).unwrap(),
                nullable: true
            },
            scalar(ScalarType::Decimal, false)
        ]
    );
    source.allowed.set(true);
    let result = plan.execute(&source, &[], Default::default()).unwrap();
    assert_eq!(
        result.rows(),
        &[
            vec![
                Value::Text("é🦀".into()),
                Value::Decimal(Decimal::new(100, 2).unwrap())
            ],
            vec![
                Value::Text("hello".into()),
                Value::Decimal(Decimal::new(1000, 3).unwrap())
            ],
        ]
    );
    let result = prepare(
        &syntax("SELECT X AS N FROM T UNION SELECT Y FROM T"),
        &source,
    )
    .unwrap()
    .execute(&source, &[], Default::default())
    .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Decimal(Decimal::new(100, 2).unwrap())]]
    );
}

#[test]
fn unresolved_families_and_expression_states_keep_their_existing_carriers() {
    for sql in [
        "SELECT #2024-01-02# AS N UNION ALL SELECT 1",
        "SELECT #2024-01-02# AS N UNION ALL SELECT 'date'",
    ] {
        assert_eq!(run(sql).output_types(), [OutputType::Dynamic], "{sql}");
    }
    assert_eq!(
        run("SELECT TypeName(U.N) FROM (SELECT CByte(1) AS N UNION ALL SELECT CDbl(2)) AS U")
            .rows(),
        &[
            vec![Value::Text("Double".into())],
            vec![Value::Text("Double".into())]
        ]
    );
    let source = MemorySource::new(vec![]).unwrap();
    let plan = prepare(
        &syntax("SELECT CInt(1) AS N UNION ALL SELECT CVErr(5)"),
        &source,
    )
    .unwrap();
    let result = plan
        .execute_expression(&source, &[], Default::default())
        .unwrap();
    assert!(matches!(result.rows()[1][0], ExpressionValue::Error(_)));
    assert_eq!(
        result.rows()[0],
        vec![ExpressionValue::Value(Value::Int16(1))]
    );
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NonScalarExpression))
    );
    // Conditional branch inference and evaluation do not adopt UNION widening.
    let plan = prepare(
        &syntax("SELECT IIf(CInt(1)=CInt(1),CInt(1),CLng(2))"),
        &source,
    )
    .unwrap();
    assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int16(1)]]
    );
}

#[test]
fn widening_keeps_late_errors_cancellation_and_work_allocation_row_limits() {
    let source = MemorySource::new(vec![]).unwrap();
    let plan = prepare(
        &syntax("SELECT CByte(1) AS N UNION ALL SELECT CLng(2) UNION ALL SELECT 1/0"),
        &source,
    )
    .unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    let plan = prepare(
        &syntax("SELECT CByte(1) AS N UNION SELECT CLng(2) UNION ALL SELECT CLngLng(3) ORDER BY N"),
        &source,
    )
    .unwrap();
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    execute().unwrap();
    for checkpoint in 1..=calls.get() {
        calls.set(0);
        stop.set(checkpoint);
        assert!(matches!(
            execute(),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            rows: 1,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(QueryError::Limit(_))
        ));
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x554e494f4e545950),
        ..Default::default()
    })]
    #[test]
    fn integral_union_rows_match_exact_signed_widening_and_set_models(
        small in proptest::prelude::any::<i16>(),
        wide in proptest::prelude::any::<i64>(),
        all in proptest::prelude::any::<bool>(),
    ) {
        let source = MemorySource::new(vec![]).unwrap();
        let sql = format!(
            "PARAMETERS p SHORT,q BIGINT; SELECT p AS N UNION {} SELECT q ORDER BY N",
            if all { "ALL" } else { "" },
        );
        let parsed = syntax(&sql);
        let plan = prepare(&parsed, &source).unwrap();
        let parameters = [("p", Value::Int16(small)), ("q", Value::Int64(wide))];
        let result = plan
            .execute(&source, &parameters, Default::default())
            .unwrap();
        let mut expected = vec![i64::from(small), wide];
        expected.sort();
        if !all {
            expected.dedup();
        }
        let expected = expected
            .into_iter()
            .map(|value| vec![Value::Int64(value)])
            .collect::<Vec<_>>();
        proptest::prop_assert_eq!(result.rows(), expected);
        proptest::prop_assert_eq!(result.output_types(), [scalar(ScalarType::Int64, true)]);
    }
}
