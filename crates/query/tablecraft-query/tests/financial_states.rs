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
fn financial_error_data_does_not_hide_later_eager_argument_failures() {
    for sql in [
        "SELECT SLN(CVErr(1),1/0,10)",
        "SELECT Pmt(CVErr(1),10,1/0)",
        "SELECT EuroConvert(CVErr(1),'DEM','EUR',1/0)",
        "SELECT SLN(CVErr(Q.N),1/(Q.N-1),10) FROM (SELECT 1 AS N) AS Q",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
}

#[test]
fn financial_data_admission_preserves_conversion_and_currency_error_order() {
    for (sql, error) in [
        (
            "SELECT SLN(NULL,CVErr(1),10)",
            EvalError::InvalidNullConversion,
        ),
        ("SELECT SLN(CVErr(1),NULL,10)", EvalError::TypeMismatch),
        ("SELECT SLN(-1,CVErr(1),0)", EvalError::TypeMismatch),
        ("SELECT EuroConvert(1,'USD',CVErr(1))", EvalError::Domain),
        (
            "SELECT EuroConvert(1,CVErr(1),'USD')",
            EvalError::TypeMismatch,
        ),
        ("SELECT Rate(10,-10,CVErr(1))", EvalError::TypeMismatch),
    ] {
        assert_eq!(run(sql).err(), Some(error.into()), "{sql}");
    }
    assert_eq!(
        run("SELECT SLN(CLng(CVErr(1)),0,1)").unwrap(),
        [vec![Value::Double(Double::from_float(1.0))]]
    );
}

#[test]
fn financial_plans_reuse_after_data_admission_failure_without_caching_arguments() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG; SELECT SLN(IIf(N=0,CVErr(1),100),0,10)",
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
        &[vec![Value::Double(Double::from_float(10.0))]]
    );
}
