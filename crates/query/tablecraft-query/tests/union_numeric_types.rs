use std::cell::Cell;
use tablecraft_core::values::{Currency, Decimal, Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, Source, prepare, source::MemorySource,
};
use tablecraft_schema::ScalarType::{
    Boolean, Byte, Currency as Money, Decimal as Fixed, Double as Float, Int16, Int32, Int64,
    Single,
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::{parse_query, query::Query};

const KINDS: [ScalarType; 9] = [
    Boolean, Byte, Int16, Int32, Int64, Single, Float, Money, Fixed,
];
const EXPRESSIONS: [&str; 9] = [
    "CBool(1)",
    "CByte(255)",
    "CInt(-32768)",
    "CLng(2147483647)",
    "CLngLng(9223372036854775807)",
    "CSng(1.25)",
    "CDbl(2.5)",
    "CCur(1.25)",
    "CDec(2.5)",
];
const RESULTS: [[ScalarType; 9]; 9] = [
    [
        Boolean, Int16, Int16, Int32, Int64, Float, Float, Money, Fixed,
    ],
    [Int16, Byte, Int16, Int32, Int64, Float, Float, Money, Fixed],
    [
        Int16, Int16, Int16, Int32, Int64, Float, Float, Money, Fixed,
    ],
    [
        Int32, Int32, Int32, Int32, Int64, Float, Float, Money, Fixed,
    ],
    [
        Int64, Int64, Int64, Int64, Int64, Float, Float, Fixed, Fixed,
    ],
    [
        Float, Float, Float, Float, Float, Single, Float, Float, Float,
    ],
    [
        Float, Float, Float, Float, Float, Float, Float, Float, Float,
    ],
    [
        Money, Money, Money, Money, Fixed, Float, Float, Money, Fixed,
    ],
    [
        Fixed, Fixed, Fixed, Fixed, Fixed, Float, Float, Fixed, Fixed,
    ],
];

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
        .execute(&source, &[], Default::default())
        .unwrap()
}

#[test]
fn complete_numeric_matrix_has_one_validated_carrier_in_both_arm_orders() {
    for (a, left) in EXPRESSIONS.iter().enumerate() {
        for (b, right) in EXPRESSIONS.iter().enumerate() {
            let result = run(&format!("SELECT {left} AS N UNION ALL SELECT {right}"));
            let expected = RESULTS[a][b];
            assert_eq!(
                result.output_types(),
                [scalar(expected, true)],
                "{left}, {right}"
            );
            assert_eq!(result.rows().len(), 2);
            for row in result.rows() {
                assert!(
                    TypeDescriptor::new(expected)
                        .validate_value(&row[0])
                        .is_ok(),
                    "{left}, {right}: {:?}",
                    row[0]
                );
            }
        }
    }
}

#[test]
fn fixed_point_unions_keep_exact_wide_integers_and_currency_coefficients() {
    assert_eq!(
        run("SELECT CLngLng(9223372036854775807) AS N UNION ALL SELECT CCur(1.25)").rows(),
        [
            vec![Value::Decimal(
                Decimal::new(i128::from(i64::MAX), 0).unwrap()
            )],
            vec![Value::Decimal(Decimal::new(12500, 4).unwrap())],
        ]
    );
    assert_eq!(
        run("SELECT True AS N UNION ALL SELECT CByte(255)").rows(),
        [vec![Value::Int16(-1)], vec![Value::Int16(255)]]
    );
    assert_eq!(
        run("SELECT CLng(2147483647) AS N UNION ALL SELECT CCur(1.25)").rows(),
        [
            vec![Value::Currency(Currency::from_scaled(21474836470000))],
            vec![Value::Currency(Currency::from_scaled(12500))],
        ]
    );
    assert_eq!(
        run("SELECT CCur(1) AS N UNION SELECT CDec(1)").rows(),
        [vec![Value::Decimal(Decimal::new(10000, 4).unwrap())]]
    );
}

#[test]
fn double_result_rounds_before_distinct_and_keeps_the_same_contract_for_all() {
    let wide = "SELECT CLngLng(9007199254740993) AS N UNION SELECT CDbl(9007199254740992)";
    assert_eq!(
        run(wide).rows(),
        [vec![Value::Double(Double::from_float(9007199254740992.0))]]
    );
    let all = wide.replace("UNION SELECT", "UNION ALL SELECT");
    assert_eq!(
        run(&all).rows(),
        [
            vec![Value::Double(Double::from_float(9007199254740992.0))],
            vec![Value::Double(Double::from_float(9007199254740992.0))]
        ]
    );
    assert_eq!(
        run("SELECT TypeName(N),N FROM (SELECT CInt(1) AS N UNION ALL SELECT CDbl(2)) AS Q").rows(),
        [
            vec![
                Value::Text("Double".into()),
                Value::Double(Double::from_float(1.0))
            ],
            vec![
                Value::Text("Double".into()),
                Value::Double(Double::from_float(2.0))
            ],
        ]
    );
}

struct MetadataOnly {
    columns: Vec<Column>,
}
impl Source for MetadataOnly {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("type selection cannot scan")
    }
}
#[test]
fn metadata_type_combination_is_associative_includes_empty_arms_and_never_scans() {
    let source = MetadataOnly {
        columns: KINDS
            .iter()
            .enumerate()
            .map(|(index, kind)| Column::new(format!("N{index}"), *kind, index % 2 == 0).unwrap())
            .collect(),
    };
    for a in 0..KINDS.len() {
        for b in 0..KINDS.len() {
            for c in 0..KINDS.len() {
                let first = prepare(&syntax(&format!("SELECT N{a} FROM T UNION ALL SELECT N{b} FROM T UNION ALL SELECT N{c} FROM T")), &source).unwrap();
                let second = prepare(&syntax(&format!("SELECT N{c} FROM T UNION ALL SELECT N{a} FROM T UNION ALL SELECT N{b} FROM T")), &source).unwrap();
                assert_eq!(first.output_types(), second.output_types());
            }
        }
    }
    assert_eq!(
        run("SELECT CLngLng(9007199254740993) AS N UNION ALL SELECT CDbl(0) WHERE False").rows(),
        [vec![Value::Double(Double::from_float(9007199254740992.0))]]
    );
}

#[test]
fn conversion_retains_late_failures_and_every_observed_cancellation_checkpoint() {
    let source = MemorySource::new(vec![]).unwrap();
    let plan = prepare(&syntax("SELECT True AS N UNION SELECT CCur(1) UNION ALL SELECT CLngLng(2) UNION ALL SELECT CDbl(3) ORDER BY N"), &source).unwrap();
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
    let plan = prepare(
        &syntax("SELECT CCur(1) AS N UNION ALL SELECT CDbl(2) UNION ALL SELECT 1/0"),
        &source,
    )
    .unwrap();
    assert_eq!(
        plan.execute(&source, &[], Default::default()).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4d49584544554e49),
        ..Default::default()
    })]
    #[test]
    fn wide_integer_results_follow_exact_decimal_or_ieee_destination_models(
        integer in proptest::prelude::any::<i64>(),
        coefficient in proptest::prelude::any::<i64>(),
    ) {
        let source = MemorySource::new(vec![]).unwrap();
        let query = syntax("PARAMETERS p BIGINT,q CURRENCY; SELECT p AS N UNION ALL SELECT q");
        let parameters = [
            ("p", Value::Int64(integer)),
            ("q", Value::Currency(Currency::from_scaled(coefficient))),
        ];
        let result = prepare(&query, &source)
            .unwrap()
            .execute(&source, &parameters, Default::default())
            .unwrap();
        proptest::prop_assert_eq!(result.rows(), [
            vec![Value::Decimal(Decimal::new(i128::from(integer), 0).unwrap())],
            vec![Value::Decimal(Decimal::new(i128::from(coefficient), 4).unwrap())],
        ]);
        let query = syntax("PARAMETERS p BIGINT; SELECT p AS N UNION ALL SELECT CDbl(0)");
        let result = prepare(&query, &source)
            .unwrap()
            .execute(&source, &parameters[..1], Default::default())
            .unwrap();
        proptest::prop_assert_eq!(result.rows(), [
            vec![Value::Double(Double::from_float(integer as f64))],
            vec![Value::Double(Double::from_float(0.0))],
        ]);
    }
}
