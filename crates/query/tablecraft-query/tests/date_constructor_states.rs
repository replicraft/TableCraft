use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{EvalError, functions::dates::TwoDigitYearWindow};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
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
fn constructor_data_admission_keeps_eager_expressions_and_ordered_component_errors() {
    for (sql, error) in [
        (
            "SELECT DateSerial(CVErr(1),1/0,1)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT TimeSerial(CVErr(1),0,1/0)",
            EvalError::DivisionByZero,
        ),
        (
            "SELECT DateSerial(NULL,CVErr(1),1)",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT DateSerial(CVErr(1),NULL,1)",
            EvalError::TypeMismatch,
        ),
        ("SELECT DateSerial(32768,CVErr(1),1)", EvalError::Overflow),
        (
            "SELECT DateSerial(9999,13,CVErr(1))",
            EvalError::TypeMismatch,
        ),
        (
            "SELECT DateSerial(CVErr(Q.N),1/(Q.N-1),1) FROM (SELECT 1 AS N) AS Q",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(run(sql).err(), Some(error.into()), "{sql}");
    }
    assert_eq!(
        run("SELECT Year(DateSerial(CLng(CVErr(2001)),1,1))").unwrap(),
        [vec![Value::Int16(2001)]]
    );
}

#[test]
fn constructor_plans_reuse_after_data_failure_with_execution_local_year_windows() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG; SELECT DateSerial(IIf(N=0,CVErr(1),24),1,1)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.execute(&source, &[("N", Value::Int32(0))], Default::default())
            .err(),
        Some(EvalError::TypeMismatch.into())
    );
    for (last, year) in [(2029, 2024), (1999, 1924)] {
        let expected = Value::DateTime(
            LegacyDateTime::from_calendar(format!("{year}-01-01T00:00:00").parse().unwrap())
                .unwrap(),
        );
        assert_eq!(
            plan.execute(
                &source,
                &[("N", Value::Int32(1))],
                ExecutionOptions {
                    year_window: Some(TwoDigitYearWindow::new(last).unwrap()),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            &[vec![expected]]
        );
    }
}
