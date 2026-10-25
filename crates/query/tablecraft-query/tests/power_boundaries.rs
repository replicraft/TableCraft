//! Power's converted-operand domain boundary, distinct from operand failures.
use std::cell::Cell;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, Source, prepare, prepare_with_options,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn run(sql: &str, declared: bool) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let syntax = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(syntax.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: declared,
                number_format: Some(NumberFormat::Invariant),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn every_supported_converted_zero_and_negative_carrier_uses_domain() {
    let bases = [
        "0",
        "False",
        "CByte(0)",
        "CInt(0)",
        "CLng(0)",
        "CLngLng(0)",
        "CSng(0)",
        "CDbl(0)",
        "CCur(0)",
        "CDec(0)",
        "CDate(0)",
        "'0'",
        "LeftB('0',2)",
        "(-CDbl(0))",
        "(0.5^1075)",
    ];
    let exponents = [
        "(-1)",
        "True",
        "CInt(-1)",
        "CLng(-1)",
        "CLngLng(-1)",
        "CSng(-1)",
        "CDbl(-1)",
        "CCur(-1)",
        "CDec(-1)",
        "CDate(-1)",
        "'-1'",
        "LeftB('-1',4)",
        "(-0.5)",
    ];
    for declared in [false, true] {
        for base in bases {
            for exponent in exponents {
                let sql = format!("SELECT ({base})^({exponent}) AS V");
                assert_eq!(
                    run(&sql, declared).err(),
                    Some(EvalError::Domain.into()),
                    "{sql}"
                );
            }
        }
    }
}

#[test]
fn null_raw_and_eager_operand_failures_keep_their_existing_precedence() {
    for declared in [false, true] {
        for (expression, expected) in [
            ("0^(1/0)", EvalError::DivisionByZero),
            ("NULL^(1/0)", EvalError::DivisionByZero),
            ("(0^(-1))^(1/0)", EvalError::Domain),
            ("(1/0)^(0^(-1))", EvalError::DivisionByZero),
            ("CVErr(1)^0", EvalError::TypeMismatch),
            ("0^CVErr(1)", EvalError::TypeMismatch),
            ("'bad'^(-1)", EvalError::TypeMismatch),
            ("0^'bad'", EvalError::TypeMismatch),
            ("0^ChrB(65)", EvalError::UnrepresentableText),
            ("0/0", EvalError::Overflow),
            ("CDec(0)/0", EvalError::DivisionByZero),
            ("1/0", EvalError::DivisionByZero),
        ] {
            let sql = format!("SELECT {expression}");
            assert_eq!(run(&sql, declared).err(), Some(expected.into()), "{sql}");
        }
        for expression in ["NULL^(-1)", "0^NULL", "ChrB(65)^NULL", "NULL^ChrW(55296)"] {
            assert_eq!(
                run(&format!("SELECT {expression}"), declared).unwrap(),
                [vec![Value::Null]]
            );
        }
    }
}

#[test]
fn double_operand_rounding_precedes_power_and_finite_results_keep_double() {
    for declared in [false, true] {
        for expression in [
            "(-1)^CLngLng('9007199254740993')",
            "(-1)^CDec('9007199254740993')",
            "(-1)^'9007199254740993'",
            "(-1)^LeftB('9007199254740993',32)",
        ] {
            assert_eq!(
                run(&format!("SELECT {expression}"), declared).unwrap(),
                [vec![Value::Double(Double::from_float(1.0))]]
            );
        }
        assert_eq!(
            run("SELECT 0^0,(-2)^(-3),4^0.5,VarType(2^3)", declared).unwrap(),
            [vec![
                Value::Double(Double::from_float(1.0)),
                Value::Double(Double::from_float(-0.125)),
                Value::Double(Double::from_float(2.0)),
                Value::Int16(5),
            ]]
        );
    }
}

#[test]
fn power_errors_survive_composition_without_becoming_null_or_empty_results() {
    for declared in [false, true] {
        for sql in [
            "SELECT V^(-1) FROM (SELECT 0 AS V) AS Q",
            "SELECT Sum(V^(-1)) FROM (SELECT 0 AS V) AS Q",
            "SELECT First(V)^(-1) FROM (SELECT 0 AS V) AS Q",
            "SELECT (SELECT Q.V^(-1)) FROM (SELECT 0 AS V) AS Q",
            "SELECT 1 AS V UNION ALL SELECT 0^(-1)",
            "SELECT DISTINCT V^(-1) FROM (SELECT 0 AS V) AS Q",
            "SELECT V FROM (SELECT 0 AS V) AS Q ORDER BY V^(-1)",
            "TRANSFORM First(V)^(-1) SELECT True AS G FROM (SELECT 0 AS V) AS Q GROUP BY True PIVOT V IN (0)",
        ] {
            assert_eq!(
                run(sql, declared).err(),
                Some(EvalError::Domain.into()),
                "{sql}"
            );
        }
    }
}

#[test]
fn prepared_parameters_rebind_without_caching_success_and_cancellation_remains_explicit() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "PARAMETERS B DOUBLE,E DOUBLE; SELECT B^E",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for declared in [false, true] {
        for (base, exponent, expected) in [
            (
                0.0,
                0.0,
                Ok(vec![vec![Value::Double(Double::from_float(1.0))]]),
            ),
            (0.0, -1.0, Err(QueryError::Evaluation(EvalError::Domain))),
            (
                2.0,
                3.0,
                Ok(vec![vec![Value::Double(Double::from_float(8.0))]]),
            ),
        ] {
            let result = plan
                .execute(
                    &source,
                    &[
                        ("B", Value::Double(Double::from_float(base))),
                        ("E", Value::Double(Double::from_float(exponent))),
                    ],
                    ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    },
                )
                .map(|result| result.rows().to_vec());
            assert_eq!(result, expected);
        }
    }

    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        true
    };
    let result = plan.execute(
        &source,
        &[
            ("B", Value::Double(Double::from_float(0.0))),
            ("E", Value::Double(Double::from_float(-1.0))),
        ],
        ExecutionOptions {
            cancelled: Some(&cancelled),
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::Cancelled));
    assert!(calls.get() > 0);
}

#[test]
fn runtime_domain_aggregate_preserves_power_errors() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(0)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let syntax = parse_query("SELECT DSum('N^(-1)','T')", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for declared in [false, true] {
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: declared,
                    ..Default::default()
                }
            )
            .err(),
            Some(EvalError::Domain.into())
        );
    }
}

#[test]
fn named_crosstab_constant_heading_reports_domain_before_any_scan() {
    struct MetadataOnly {
        definition: tablecraft_sql::query::Query,
        columns: Vec<Column>,
        scans: Cell<usize>,
    }

    impl Source for MetadataOnly {
        fn query(&self, name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
            Ok((name == "P").then_some(&self.definition))
        }

        fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
            assert_eq!(table, "T");
            Ok(&self.columns)
        }

        fn scan(
            &self,
            _table: &str,
            _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            self.scans.set(self.scans.get() + 1);
            Err(QueryError::Source)
        }
    }

    let definition = parse_query(
        "TRANSFORM Sum(N) SELECT G FROM T GROUP BY G PIVOT N IN (0^(-1))",
        Default::default(),
    )
    .unwrap()
    .into_query();
    let source = MetadataOnly {
        definition,
        columns: vec![
            Column::new("G".into(), ScalarType::Int32, false).unwrap(),
            Column::new("N".into(), ScalarType::Int32, false).unwrap(),
        ],
        scans: Cell::new(0),
    };
    let syntax = parse_query("SELECT * FROM P", Default::default()).unwrap();
    for declared in [false, true] {
        assert_eq!(
            prepare_with_options(
                syntax.query(),
                &source,
                PreparationOptions {
                    declared_arithmetic: declared,
                    ..Default::default()
                }
            )
            .err(),
            Some(EvalError::Domain.into())
        );
        assert_eq!(source.scans.get(), 0);
    }
}
