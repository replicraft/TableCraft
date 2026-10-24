use tablecraft_core::values::Value;
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
fn math_error_data_does_not_hide_later_operational_argument_failures() {
    for sql in [
        "SELECT RGB(CVErr(1),1/0,0)",
        "SELECT Round(CVErr(1),1/0)",
        "SELECT Round(IIf(True,CVErr(1),0),1/0)",
        "SELECT Sqr(IIf(True,CVErr(1),1/0))",
        "SELECT RGB(CVErr(Q.N),1/(Q.N-1),0) FROM (SELECT 1 AS N) AS Q",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
}

#[test]
fn math_data_admission_retains_count_and_channel_validation_precedence() {
    for (sql, error) in [
        ("SELECT Abs(CVErr(1))", EvalError::TypeMismatch),
        ("SELECT Sqr(CVErr(1))", EvalError::TypeMismatch),
        ("SELECT Round(CVErr(1),-1)", EvalError::Domain),
        (
            "SELECT Round(CVErr(1),NULL)",
            EvalError::InvalidNullConversion,
        ),
        ("SELECT Round(NULL,CVErr(1))", EvalError::TypeMismatch),
        ("SELECT RGB(-1,CVErr(1),0)", EvalError::Domain),
        ("SELECT RGB(CVErr(1),-1,0)", EvalError::TypeMismatch),
        ("SELECT QBColor(CVErr(1))", EvalError::TypeMismatch),
    ] {
        assert_eq!(run(sql).err(), Some(error.into()), "{sql}");
    }
    assert_eq!(
        run("SELECT Abs(CLng(CVErr(1)))").unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(
        run("SELECT Abs(First(CVErr(Q.N))) FROM (SELECT 1 AS N) AS Q").err(),
        Some(EvalError::TypeMismatch.into())
    );
}
