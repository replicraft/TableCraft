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
fn single_date_states_retain_week_policy_and_eager_argument_precedence() {
    for (sql, error) in [
        ("SELECT Weekday(CVErr(1),1/0)", EvalError::DivisionByZero),
        ("SELECT Weekday(CVErr(1),8)", EvalError::Domain),
        (
            "SELECT Weekday(CVErr(1),0)",
            EvalError::CalendarPolicyRequired,
        ),
        ("SELECT Weekday(NULL,CVErr(1))", EvalError::TypeMismatch),
        (
            "SELECT Weekday(Q.E,1/(Q.N-1)) FROM (SELECT CVErr(1) AS E,1 AS N) AS Q",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(run(sql).err(), Some(error.into()), "{sql}");
    }
}

#[test]
fn single_date_consumers_refuse_implicit_error_codes_through_derived_values() {
    for function in [
        "CDate",
        "CVDate",
        "DateValue",
        "TimeValue",
        "Year",
        "Month",
        "Day",
        "Hour",
        "Minute",
        "Second",
        "Weekday",
    ] {
        let sql = format!("SELECT {function}(Q.E) FROM (SELECT CVErr(1) AS E) AS Q");
        assert_eq!(
            run(&sql).err(),
            Some(EvalError::TypeMismatch.into()),
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT Day(CDate(CLng(CVErr(1)))),Weekday(NULL),DateValue(NULL),TimeValue(NULL)")
            .unwrap(),
        [vec![
            Value::Int16(31),
            Value::Null,
            Value::Null,
            Value::Null
        ]]
    );
}

#[test]
fn single_date_parameter_plan_reuses_after_error_state_failure() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG; SELECT Year(IIf(N=0,CVErr(1),0)),Weekday(IIf(N=0,CVErr(1),0))",
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
        &[vec![Value::Int16(1899), Value::Int16(7)]]
    );
}
