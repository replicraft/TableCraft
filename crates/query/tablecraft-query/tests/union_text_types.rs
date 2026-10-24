use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryLimits, Source, prepare,
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
        collation: Some(&Ordinal),
        ..Default::default()
    }
}
fn text(nullable: bool) -> OutputType {
    OutputType::Scalar {
        data_type: TypeDescriptor::new(ScalarType::Text),
        nullable,
    }
}

#[test]
fn every_numeric_family_combines_with_text_in_both_orders_under_current_format() {
    let source = MemorySource::new(vec![]).unwrap();
    for (input, spelling) in [
        ("CBool(1)", "True"),
        ("CByte(255)", "255"),
        ("CInt(-32768)", "-32768"),
        ("CLng(2147483647)", "2147483647"),
        ("CLngLng(9007199254740993)", "9007199254740993"),
        ("CSng(1.25)", "1.25"),
        ("CDbl(2.5)", "2.5"),
        ("CCur(1.25)", "1.25"),
        ("CDec(2.5)", "2.5"),
    ] {
        for reverse in [false, true] {
            let sql = if reverse {
                format!("SELECT 'tail' AS N UNION ALL SELECT {input}")
            } else {
                format!("SELECT {input} AS N UNION ALL SELECT 'tail'")
            };
            let plan = prepare(&syntax(&sql), &source).unwrap();
            assert_eq!(plan.output_types().unwrap(), [text(false)]);
            for (format, expected) in [
                (NumberFormat::EnUs, spelling.to_owned()),
                (NumberFormat::DeDe, spelling.replace('.', ",")),
            ] {
                let result = plan
                    .execute(
                        &source,
                        &[],
                        ExecutionOptions {
                            number_format: Some(format),
                            ..options()
                        },
                    )
                    .unwrap();
                let mut rows = vec![
                    vec![Value::Text(expected)],
                    vec![Value::Text("tail".into())],
                ];
                if reverse {
                    rows.reverse();
                }
                assert_eq!(result.rows(), rows, "{sql}");
            }
        }
    }
}

#[test]
fn conversion_precedes_duplicates_and_global_text_selection_preserves_integer_digits() {
    let source = MemorySource::new(vec![]).unwrap();
    for (sql, expected) in [
        (
            "SELECT 1 AS N UNION SELECT '1'",
            vec![vec![Value::Text("1".into())]],
        ),
        (
            "SELECT 1 AS N UNION ALL SELECT '1'",
            vec![vec![Value::Text("1".into())], vec![Value::Text("1".into())]],
        ),
        (
            "SELECT 9007199254740993 AS N UNION ALL SELECT CDbl(0) UNION ALL SELECT 'tail'",
            vec![
                vec![Value::Text("9007199254740993".into())],
                vec![Value::Text("0".into())],
                vec![Value::Text("tail".into())],
            ],
        ),
        (
            "SELECT CCur(1.25) AS N UNION SELECT '1.25'",
            vec![vec![Value::Text("1.25".into())]],
        ),
    ] {
        let plan = prepare(&syntax(sql), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], options()).unwrap().rows(),
            expected,
            "{sql}"
        );
    }
    let query = syntax("SELECT CCur(1.25) AS N UNION SELECT '1.25'");
    let result = prepare(&query, &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..options()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Text("1,25".into())],
            vec![Value::Text("1.25".into())]
        ]
    );
}

struct NoScan {
    columns: Vec<Column>,
    named: Query,
}
impl Source for NoScan {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "V").then_some(&self.named))
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("UNION typing must not scan")
    }
}
#[test]
fn metadata_drops_text_constraints_and_survives_named_null_and_empty_arms() {
    let source = NoScan {
        columns: vec![
            Column::with_type("T".into(), TypeDescriptor::text(1).unwrap(), false).unwrap(),
            Column::new("N".into(), ScalarType::Int64, false).unwrap(),
        ],
        named: syntax("SELECT T AS X FROM Data UNION ALL SELECT N FROM Data"),
    };
    assert_eq!(
        prepare(&syntax("SELECT * FROM V"), &source)
            .unwrap()
            .output_types()
            .unwrap(),
        [text(false)]
    );
    assert_eq!(
        prepare(
            &syntax("SELECT N FROM Data UNION ALL SELECT T FROM Data UNION ALL SELECT Null"),
            &source
        )
        .unwrap()
        .output_types()
        .unwrap(),
        [text(true)]
    );
    let empty = MemorySource::new(vec![]).unwrap();
    let plan = prepare(
        &syntax("SELECT 1 AS N WHERE False UNION ALL SELECT 'x'"),
        &empty,
    )
    .unwrap();
    assert_eq!(
        plan.execute(&empty, &[], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Text("x".into())]]
    );
    let plan = prepare(
        &syntax("SELECT 1 AS N UNION ALL SELECT 'x' WHERE False"),
        &empty,
    )
    .unwrap();
    assert_eq!(
        plan.execute(&empty, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
}

#[test]
fn nulls_stay_null_and_conversion_or_comparison_policies_remain_explicit() {
    let source = MemorySource::new(vec![]).unwrap();
    let query = syntax("PARAMETERS P LONG; SELECT P AS N UNION ALL SELECT 'x'");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[("P", Value::Null)], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Null], vec![Value::Text("x".into())]]
    );
    assert_eq!(
        plan.execute(&source, &[("P", Value::Int32(1))], Default::default())
            .err(),
        Some(QueryError::Evaluation(EvalError::NumberFormatRequired))
    );
    assert_eq!(
        plan.execute(&source, &[("P", Value::Int32(2))], options())
            .unwrap()
            .rows(),
        [vec![Value::Text("2".into())], vec![Value::Text("x".into())]]
    );
    let plan = prepare(&syntax("SELECT 1 AS N UNION SELECT '1'"), &source).unwrap();
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
    let plan = prepare(&syntax("SELECT True AS N UNION ALL SELECT 'x'"), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        [
            vec![Value::Text("True".into())],
            vec![Value::Text("x".into())]
        ]
    );
}

#[test]
fn all_observed_cancellation_points_and_late_arm_failures_remain_visible() {
    let source = MemorySource::new(vec![]).unwrap();
    let plan = prepare(&syntax("SELECT 9007199254740993 AS N UNION SELECT '9007199254740993' UNION ALL SELECT CCur(1.25) ORDER BY N"), &source).unwrap();
    let calls = Cell::new(0);
    let execute = |stop| {
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
                ..options()
            },
        )
    };
    assert!(execute(usize::MAX).is_ok());
    let count = calls.get();
    for stop in 1..=count {
        assert!(
            matches!(
                execute(stop),
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    let plan = prepare(
        &syntax("SELECT 1 AS N UNION ALL SELECT 'x' UNION ALL SELECT 1/0"),
        &source,
    )
    .unwrap();
    assert_eq!(
        plan.execute(&source, &[], options()).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn numeric_rendering_charges_its_payload_to_the_shared_query_budget() {
    let source = MemorySource::new(vec![]).unwrap();
    let query = syntax("PARAMETERS P BIGINT; SELECT P AS N UNION ALL SELECT 'x'");
    let plan = prepare(&query, &source).unwrap();
    let minimum = |value| {
        let (mut low, mut high) = (0, 65_536);
        while low < high {
            let bytes = (low + high) / 2;
            let result = plan.execute_expression(
                &source,
                &[("P", Value::Int64(value))],
                ExecutionOptions {
                    limits: QueryLimits {
                        bytes,
                        ..Default::default()
                    },
                    ..options()
                },
            );
            if result.is_ok() {
                high = bytes;
            } else {
                low = bytes + 1;
            }
        }
        low
    };
    assert_eq!(minimum(1_000_000_000_000_000_000) - minimum(1), 18);
}

#[test]
fn converted_text_flows_through_outer_groups_and_fixed_crosstabs() {
    let source = MemorySource::new(vec![]).unwrap();
    let plan = prepare(
        &syntax("SELECT N,Count(*) FROM (SELECT 1 AS N UNION ALL SELECT '1') AS Q GROUP BY N"),
        &source,
    )
    .unwrap();
    assert_eq!(
        plan.execute(&source, &[], options()).unwrap().rows(),
        [vec![Value::Text("1".into()), Value::Int64(2)]]
    );
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(1)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let plan = prepare(&syntax("TRANSFORM Count(*) SELECT True AS G FROM (SELECT ID AS N FROM T UNION ALL SELECT '1') AS Q GROUP BY True PIVOT N IN ('1')"), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[], options()).unwrap().rows(),
        [vec![Value::Boolean(true), Value::Int64(2)]]
    );
    let query = syntax("SELECT 1 AS N UNION ALL SELECT CVErr(1) UNION ALL SELECT 'x'");
    let plan = prepare(&query, &source).unwrap();
    assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic]);
    assert!(matches!(
        plan.execute_expression(&source, &[], options())
            .unwrap()
            .rows()[1][0],
        ExpressionValue::Error(1)
    ));
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x54455854554e494f),
        ..Default::default()
    })]
    #[test]
    fn signed_integer_rendering_is_exact_before_text_duplicate_removal(
        integer in proptest::prelude::any::<i64>(),
        all in proptest::prelude::any::<bool>(),
    ) {
        let source = MemorySource::new(vec![]).unwrap();
        let spelling = integer.to_string();
        let query = syntax(&format!(
            "PARAMETERS P BIGINT,S TEXT; SELECT P AS N UNION {} SELECT S",
            if all { "ALL" } else { "" },
        ));
        let plan = prepare(&query, &source).unwrap();
        let parameters = [("P", Value::Int64(integer)), ("S", Value::Text(spelling.clone()))];
        let result = plan.execute(&source, &parameters, options()).unwrap();
        proptest::prop_assert_eq!(result.rows(), vec![vec![Value::Text(spelling)]; if all { 2 } else { 1 }]);
        proptest::prop_assert_eq!(result.output_types(), [text(true)]);
    }
}
