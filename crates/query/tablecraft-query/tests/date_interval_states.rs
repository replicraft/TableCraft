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
fn date_interval_states_keep_later_operational_arguments_eager() {
    for sql in [
        "SELECT DateAdd(CVErr(1),1/0,0)",
        "SELECT DateDiff('d',CVErr(1),0,1/0)",
        "SELECT DatePart('yyyy',CVErr(1),1/0)",
        "SELECT DateAdd(CVErr(Q.N),1/(Q.N-1),0) FROM (SELECT 1 AS N) AS Q",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
}

#[test]
fn date_interval_control_and_source_validation_keeps_its_existing_order() {
    for (sql, error) in [
        (
            "SELECT DateAdd('bad',CVErr(1),0)",
            EvalError::InvalidDateInterval,
        ),
        (
            "SELECT DateDiff('bad',CVErr(1),0)",
            EvalError::InvalidDateInterval,
        ),
        (
            "SELECT DateDiff('d',CVErr(1),0,8,CVErr(1))",
            EvalError::Domain,
        ),
        (
            "SELECT DatePart('d',CVErr(1),0,CVErr(1))",
            EvalError::CalendarPolicyRequired,
        ),
        ("SELECT DateDiff('d',2958466,CVErr(1))", EvalError::Overflow),
        (
            "SELECT DateDiff('d',NULL,CVErr(1))",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT DatePart('d',CVErr(1),1,CVErr(1))",
            EvalError::TypeMismatch,
        ),
    ] {
        assert_eq!(run(sql).err(), Some(error.into()), "{sql}");
    }
    assert_eq!(
        run("SELECT Day(DateAdd('d',CLng(CVErr(1)),0))").unwrap(),
        [vec![Value::Int16(31)]]
    );
}

#[test]
fn interval_parameter_plans_reuse_after_date_state_failure() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG; SELECT DatePart('yyyy',IIf(N=0,CVErr(1),0))",
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
        &[vec![Value::Int16(1899)]]
    );
}
