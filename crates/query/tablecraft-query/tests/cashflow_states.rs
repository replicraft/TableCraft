use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn cashflow_data_admission_keeps_all_expressions_eager_and_conversion_ordered() {
    for (sql, error) in [
        ("SELECT NPV(CVErr(1),Array(1/0))", EvalError::DivisionByZero),
        (
            "SELECT NPV(0,Array(CVErr(1),1/0))",
            EvalError::DivisionByZero,
        ),
        ("SELECT IRR(Array(CVErr(1)),1/0)", EvalError::DivisionByZero),
        (
            "SELECT MIRR(Array(CVErr(1)),1/0,0)",
            EvalError::DivisionByZero,
        ),
        ("SELECT NPV(CVErr(1),Array(NULL))", EvalError::TypeMismatch),
        (
            "SELECT IRR(Array(NULL),CVErr(1))",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT NPV(0,Array(NULL,CVErr(1)))",
            EvalError::InvalidNullConversion,
        ),
        ("SELECT NPV(0,Array(CVErr(0)))", EvalError::TypeMismatch),
        (
            "SELECT NPV(0,Array(CVErr(Q.N),1/(Q.N-1))) FROM (SELECT 1 AS N) AS Q",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(run(sql).err(), Some(error.into()), "{sql}");
    }
    assert_eq!(
        run("SELECT NPV(0,Array(CLng(CVErr(1))))").unwrap(),
        [vec![Value::Double(Double::from_float(1.0))]]
    );
}

#[test]
fn cashflow_parameter_plans_recover_after_element_admission_failure() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG; SELECT NPV(0,Array(IIf(N=0,CVErr(1),100)))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[("N", Value::Int32(0))], Default::default())
            .err(),
        Some(EvalError::TypeMismatch.into())
    );
    assert_eq!(
        plan.execute(&source, &[("N", Value::Int32(1))], Default::default())
            .unwrap()
            .rows(),
        &[vec![Value::Double(Double::from_float(100.0))]]
    );
}
