use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn round_long_precision_rebinds_in_direct_derived_and_scalar_subquery_outputs() {
    let source = MemorySource::default();
    for body in [
        "SELECT Round(P,Places) AS N",
        "SELECT Q.N FROM (SELECT Round(P,Places) AS N) AS Q",
        "SELECT (SELECT Round(P,Places)) AS N",
        "SELECT Round(P,Places) AS N UNION ALL SELECT Round(P,Places)",
    ] {
        let sql = format!("PARAMETERS P VALUE,Places LONG; {body}");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        for (input, places, expected) in [
            (1e-30, 29, 0.0),
            (1e-30, 30, 1e-30),
            (f64::from_bits(3), 323, f64::from_bits(2)),
            (f64::from_bits(1), 324, f64::from_bits(1)),
            (-0.0, i32::MAX, -0.0),
        ] {
            let parameters = [("P", double(input)), ("Places", Value::Int32(places))];
            let result = plan
                .execute(&source, &parameters, Default::default())
                .unwrap();
            assert!(!result.rows().is_empty());
            for row in result.rows() {
                assert_eq!(row, &[double(expected)], "{sql}/{places}");
            }
        }
    }
}

#[test]
fn round_high_precision_keeps_null_and_error_admission_and_eager_failures() {
    let source = MemorySource::default();
    for (expression, expected) in [
        ("Round(CVErr(1),29)", EvalError::TypeMismatch),
        ("Round(CVErr(1),-1)", EvalError::Domain),
        ("Round(1/0,2147483647)", EvalError::DivisionByZero),
        ("Round(NULL,2147483648)", EvalError::Overflow),
    ] {
        let parsed = parse_query(&format!("SELECT {expression}"), Default::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .unwrap()
                .execute(&source, &[], ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{expression}"
        );
    }
    let parsed = parse_query(
        "SELECT N,VarType(N) FROM (SELECT Round(NULL,2147483647) AS N UNION ALL SELECT CByte(2)) AS Q",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap()
            .rows(),
        &[
            vec![Value::Null, Value::Int16(1)],
            vec![Value::Byte(2), Value::Int16(17)]
        ]
    );
}
