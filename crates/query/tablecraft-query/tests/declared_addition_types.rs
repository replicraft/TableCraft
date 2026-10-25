//! Known mixed-plus declarations describe successful carriers without evaluating operands.
use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    comparison::Collation,
    functions::{conversions::text::NumberFormat, dates::parsing::DateTextFormat},
};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        date_format: Some(DateTextFormat::Iso),
        collation: Some(&Ordinal),
        ..Default::default()
    }
}

fn kind(output: OutputType) -> Option<ScalarType> {
    match output {
        OutputType::Scalar { data_type, .. } => Some(data_type.scalar_type()),
        OutputType::Null | OutputType::Dynamic => None,
    }
}

#[test]
fn scalar_string_and_variant_numeric_have_text_only_in_the_declared_profile() {
    let source = MemorySource::default();
    for numeric in [
        "CBool(True)",
        "CByte(1)",
        "CInt(1)",
        "CLng(1)",
        "CLngLng(1)",
        "CSng(1)",
        "CDbl(1)",
        "CCur(1)",
        "CDec(1)",
        "CDate(1)",
    ] {
        for expression in [
            format!("'2'+CVar({numeric})"),
            format!("CVar({numeric})+'2'"),
        ] {
            let sql = format!("SELECT {expression} AS V");
            let syntax = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic], "{sql}");

            let declared = plan.execute(&source, &[], options(true)).unwrap();
            assert_eq!(
                kind(declared.output_types()[0]),
                Some(ScalarType::Text),
                "{sql}"
            );
            assert!(matches!(declared.rows()[0][0], Value::Text(_)), "{sql}");

            let materialized = plan.execute(&source, &[], options(false)).unwrap();
            assert_eq!(materialized.output_types(), [OutputType::Dynamic], "{sql}");
            assert!(
                !matches!(materialized.rows()[0][0], Value::Text(_)),
                "{sql}"
            );
        }
    }
}

#[test]
fn variant_string_numeric_addition_retains_its_guaranteed_numeric_carrier() {
    let source = MemorySource::default();
    for (numeric, expected) in [
        ("CBool(True)", Some(ScalarType::Double)),
        ("CByte(1)", Some(ScalarType::Double)),
        ("CInt(1)", Some(ScalarType::Double)),
        ("CLng(1)", Some(ScalarType::Double)),
        ("CLngLng(1)", Some(ScalarType::Double)),
        ("CSng(1)", Some(ScalarType::Double)),
        ("CDbl(1)", Some(ScalarType::Double)),
        ("CCur(1)", Some(ScalarType::Currency)),
        ("CDec(1)", Some(ScalarType::Decimal)),
        ("CDate(1)", None),
    ] {
        for peer in [numeric.to_owned(), format!("CVar({numeric})")] {
            for expression in [format!("CVar('2')+{peer}"), format!("{peer}+CVar('2')")] {
                let sql = format!("SELECT {expression} AS V");
                let syntax = parse_query(&sql, Default::default()).unwrap();
                let plan = prepare(syntax.query(), &source).unwrap();
                assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic], "{sql}");
                let declared = plan.execute(&source, &[], options(true)).unwrap();
                assert_eq!(kind(declared.output_types()[0]), expected, "{sql}");
                if let Some(expected) = expected {
                    assert_eq!(
                        ScalarType::of(&declared.rows()[0][0]),
                        Some(expected),
                        "{sql}"
                    );
                }
                let default = plan.execute(&source, &[], options(false)).unwrap();
                assert_eq!(default.output_types(), [OutputType::Dynamic], "{sql}");
            }
        }
    }
}

#[test]
fn declared_union_conversion_matches_the_explicit_string_operation_in_both_arm_orders() {
    let source = MemorySource::default();
    for expression in ["'x'+CVar(1)", "CVar(1)+'x'"] {
        for reverse in [false, true] {
            let arms = if reverse {
                format!("SELECT CLng(2) AS V UNION ALL SELECT {expression}")
            } else {
                format!("SELECT {expression} AS V UNION ALL SELECT CLng(2)")
            };
            let sql = format!("SELECT V,VarType(V) FROM ({arms}) Q");
            let syntax = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            let result = plan.execute(&source, &[], options(true)).unwrap();
            let text = if expression.starts_with("'x'") {
                "x1"
            } else {
                "1x"
            };
            let mut expected = vec![
                vec![Value::Text(text.into()), Value::Int16(8)],
                vec![Value::Text("2".into()), Value::Int16(8)],
            ];
            if reverse {
                expected.reverse();
            }
            assert_eq!(result.rows(), expected, "{sql}");
        }
    }

    // The same plan retains its old mixed runtime carriers in the default mode.
    let syntax = parse_query(
        "SELECT VarType(V) FROM (SELECT '2'+CVar(1) AS V UNION ALL SELECT CLng(2)) Q",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let result = plan.execute(&source, &[], options(false)).unwrap();
    assert_eq!(
        result.rows(),
        [vec![Value::Int16(5)], vec![Value::Int16(3)]]
    );
}

#[test]
fn nullable_grouped_and_derived_results_keep_null_and_text_without_sampling() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, true).unwrap()],
            vec![
                vec![Value::Int32(1)],
                vec![Value::Null],
                vec![Value::Int32(1)],
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let syntax = parse_query(
        "SELECT 'x'+CVar(N) AS V FROM T WHERE False",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let result = plan.execute(&source, &[], options(true)).unwrap();
    assert!(result.rows().is_empty());
    assert!(matches!(
        result.output_types()[0],
        OutputType::Scalar { data_type, nullable: true } if data_type.scalar_type() == ScalarType::Text
    ));

    let syntax = parse_query(
        "SELECT V,Count(*) FROM (SELECT 'x'+CVar(N) AS V FROM T) Q GROUP BY V",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let result = plan.execute(&source, &[], options(true)).unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Text("x1".into()), Value::Int64(2)],
            vec![Value::Null, Value::Int64(1)]
        ]
    );
}

#[test]
fn unknown_categories_nulls_and_errors_do_not_acquire_a_guessed_carrier() {
    let source = MemorySource::default();
    for (expression, error) in [
        ("'2'+First(CInt(1))", QueryError::ArithmeticDeclaration),
        (
            "'2'+CInt(1)",
            QueryError::Evaluation(EvalError::TypeMismatch),
        ),
        (
            "'x'+CVar(1/0)",
            QueryError::Evaluation(EvalError::DivisionByZero),
        ),
        (
            "'x'+CVErr(7)",
            QueryError::Evaluation(EvalError::TypeMismatch),
        ),
    ] {
        let sql = format!("SELECT {expression} AS V");
        let syntax = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic], "{sql}");
        let result = plan.execute(&source, &[], options(true));
        assert!(matches!(result, Err(actual) if actual == error), "{sql}");
    }

    // This category-independent runtime case remains usable; unknown provenance
    // still supplies no new result descriptor.
    let syntax = parse_query("SELECT CVar('2')+First(CInt(1))", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let result = plan.execute(&source, &[], options(true)).unwrap();
    assert_eq!(result.output_types(), [OutputType::Dynamic]);

    for sql in ["SELECT 'x'+CVar(NULL)", "SELECT NULL+CVar('x')"] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let result = plan.execute(&source, &[], options(true)).unwrap();
        assert_eq!(result.output_types(), [OutputType::Null]);
        assert_eq!(result.rows(), [vec![Value::Null]]);
    }

    let syntax = parse_query("SELECT 'x'+CVar(1)", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            }
        ),
        Err(QueryError::Evaluation(EvalError::NumberFormatRequired))
    ));
}
