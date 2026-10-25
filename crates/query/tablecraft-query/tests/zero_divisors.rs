use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        declared_integral: declared,
        ..Default::default()
    }
}

#[test]
fn converted_and_nested_zero_divisors_keep_operator_specific_errors() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("0/0", EvalError::Overflow),
        ("CSng(0)/CSng(-0.0)", EvalError::Overflow),
        ("CCur(0)/CDate(0)", EvalError::Overflow),
        ("CVar(CSng(0))/CSng(0)", EvalError::Overflow),
        ("'0'/CDbl(0)", EvalError::Overflow),
        ("CDec(0)/CDbl(0)", EvalError::DivisionByZero),
        ("CCur(0)/CDec(0)", EvalError::DivisionByZero),
        ("CInt(0)\\CDbl(0.5)", EvalError::DivisionByZero),
        ("CInt(0) Mod CDbl(0.5)", EvalError::DivisionByZero),
        ("NULL+(0/0)", EvalError::Overflow),
        ("IIf(True,1,0/0)", EvalError::Overflow),
        ("IsError(0/0)", EvalError::Overflow),
        (
            "(SELECT Q.N/Q.N FROM (SELECT CDbl(0) AS N) AS Q)",
            EvalError::Overflow,
        ),
        ("0/(1/0)", EvalError::DivisionByZero),
        ("(0/0)/(1/0)", EvalError::Overflow),
    ] {
        let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        for declared in [false, true] {
            let expected = match (declared, expression) {
                (true, "CDec(0)/CDbl(0)" | "CCur(0)/CDec(0)") => EvalError::Overflow,
                _ => expected,
            };
            assert_eq!(
                plan.execute(&source, &[], options(declared)).err(),
                Some(QueryError::Evaluation(expected)),
                "{expression}, declared={declared}"
            );
        }
    }
}

#[test]
fn division_failures_do_not_poison_reused_parameters_or_null_results() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N DOUBLE,D DOUBLE; SELECT N/D",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for declared in [false, true] {
        for (numerator, denominator, error) in [
            (0.0, -0.0, Some(EvalError::Overflow)),
            (1.0, 0.0, Some(EvalError::DivisionByZero)),
            (0.0, 1.0, None),
            (0.0, 0.0, Some(EvalError::Overflow)),
        ] {
            let result = plan.execute(
                &source,
                &[
                    ("N", Value::Double(Double::from_float(numerator))),
                    ("D", Value::Double(Double::from_float(denominator))),
                ],
                options(declared),
            );
            if let Some(error) = error {
                assert_eq!(result.err(), Some(QueryError::Evaluation(error)));
            } else {
                assert_eq!(
                    result.unwrap().rows(),
                    &[vec![Value::Double(Double::from_float(0.0))]]
                );
            }
        }
        assert_eq!(
            plan.execute(
                &source,
                &[
                    ("N", Value::Null),
                    ("D", Value::Double(Double::from_float(0.0)))
                ],
                options(declared)
            )
            .unwrap()
            .rows(),
            &[vec![Value::Null]]
        );
    }
}

#[test]
fn late_zero_over_zero_propagates_through_aggregates_domains_and_top() {
    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, true).unwrap()],
            vec![
                vec![Value::Int32(2)],
                vec![Value::Int32(1)],
                vec![Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap();
    let expression = "(N-1)/(N-1)";
    let mut queries = vec![format!("SELECT TOP 1 {expression} FROM T ORDER BY N DESC")];
    for aggregate in [
        "Count", "Sum", "Avg", "Min", "Max", "First", "Last", "StDev", "StDevP", "Var", "VarP",
    ] {
        queries.push(format!("SELECT {aggregate}({expression}) FROM T"));
    }
    for domain in [
        "DCount", "DSum", "DAvg", "DMin", "DMax", "DFirst", "DLast", "DStDev", "DStDevP", "DVar",
        "DVarP",
    ] {
        queries.push(format!("SELECT {domain}('{expression}','T')"));
    }
    for sql in queries {
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        for declared in [false, true] {
            assert_eq!(
                plan.execute(&source, &[], options(declared)).err(),
                Some(QueryError::Evaluation(EvalError::Overflow)),
                "{sql}, declared={declared}"
            );
        }
    }
}
