use std::cell::Cell;
use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn format_states_preserve_derived_errors_null_week_prechecks_and_eager_failures() {
    for (sql, expected) in [
        (
            "SELECT Format(E,'0',8) FROM (SELECT CVErr(11) AS E) AS D",
            EvalError::Domain,
        ),
        (
            "SELECT Format(CVErr(11),CVErr(12),8,NULL)",
            EvalError::InvalidNullConversion,
        ),
        (
            "SELECT Format(CVErr(11),NULL)",
            EvalError::InvalidNullConversion,
        ),
        ("SELECT Format(1,'0',CVErr(11))", EvalError::TypeMismatch),
        (
            "SELECT Format(CVErr(11),'0',1/0)",
            EvalError::DivisionByZero,
        ),
    ] {
        assert_eq!(run(sql), Err(QueryError::Evaluation(expected)), "{sql}");
    }
    assert_eq!(
        run("SELECT Format(CLng(CVErr(11)),'0'),Format(CStr(CVErr(11)),'@')").unwrap(),
        [vec![
            Value::Text("11".into()),
            Value::Text("Error 11".into())
        ]]
    );
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn format_retains_fields_aggregate_dependencies_group_identity_and_correlations() {
    let grouped = run("SELECT Len(Format(A.N,'0.00')),Count(*) FROM (SELECT 1 AS N UNION ALL SELECT 2) AS A GROUP BY Len(Format(A.N,'0.00'))").unwrap();
    assert_eq!(grouped, [vec![Value::Int32(4), Value::Int64(2)]]);
    let correlated = run("SELECT (SELECT Format(A.N,'000')) FROM (SELECT 12 AS N) AS A").unwrap();
    assert_eq!(correlated, [vec![Value::Text("012".into())]]);
    let aggregate =
        run("SELECT Format(Sum(A.N),'0.00') FROM (SELECT 12 AS N UNION ALL SELECT 23) AS A")
            .unwrap();
    assert_eq!(aggregate, [vec![Value::Text("35.00".into())]]);
    assert_eq!(run("SELECT Len(Format(A.N,'0.00')) FROM (SELECT 1 AS N) AS A GROUP BY Len(Format(A.N,'0000'))").err(), Some(QueryError::Grouping));
}

#[test]
fn format_parameters_and_profiles_remain_execution_local_after_binding() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS N LONG,M TEXT; SELECT Format(N,M),Format(N,,1,1)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (profile, mask, expected, plain) in [
        (
            Some(NumberFormat::EnUs),
            "#,##0.00",
            Some("1,234.00"),
            "1234",
        ),
        (
            Some(NumberFormat::DeDe),
            "#,##0.00",
            Some("1.234,00"),
            "1234",
        ),
        (None, "0.00", None, ""),
        (Some(NumberFormat::EnUs), "00000", Some("01234"), "1234"),
        (Some(NumberFormat::EnUs), "00-00", Some("12-34"), "1234"),
        (
            Some(NumberFormat::DeDe),
            "0.0\"x\"0",
            Some("1234,0x0"),
            "1234",
        ),
    ] {
        let result = plan.execute(
            &source,
            &[("N", Value::Int32(1234)), ("M", Value::Text(mask.into()))],
            ExecutionOptions {
                number_format: profile,
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [vec![
                    Value::Text(expected.into()),
                    Value::Text(plain.into())
                ]]
            );
        } else {
            assert_eq!(result.err(), Some(EvalError::NumberFormatRequired.into()));
        }
    }
}

#[test]
fn format_checks_every_observed_query_cancellation_point_and_eager_failure() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Format(-1234.567,'#,##0.00;(#,##0.00)')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    let options = ExecutionOptions {
        number_format: Some(NumberFormat::EnUs),
        cancelled: Some(&count),
        ..Default::default()
    };
    plan.execute(&source, &[], options).unwrap();
    let total = checkpoints.get();
    for stop in 1..=total {
        checkpoints.set(0);
        let cancel = || {
            checkpoints.set(checkpoints.get() + 1);
            checkpoints.get() == stop
        };
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::EnUs),
                cancelled: Some(&cancel),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }
    assert_eq!(
        run("SELECT Format(Null,'0',1/0)").err(),
        Some(EvalError::DivisionByZero.into())
    );
    assert_eq!(
        run("SELECT Format(1,'0',1,1,1/0)").err(),
        Some(QueryError::Unsupported)
    );
}

#[test]
fn parameterized_dates_reuse_plans_without_capturing_serials_or_regional_policy() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS D DateTime; SELECT Format(D,'0.00'),Format(D,'General Number'),Eval(\"Format(#1899-12-29 06:00:00#,'0.00')\")",
        Default::default(),
    ).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (serial, profile, fixed, general, dynamic) in [
        (45658.5, NumberFormat::EnUs, "45658.50", "45658.5", "-1.25"),
        (-1.25, NumberFormat::DeDe, "-1,25", "-1,25", "-1,25"),
    ] {
        let date = LegacyDateTime::from_ole_days(serial).unwrap();
        let result = plan
            .execute(
                &source,
                &[("D", Value::DateTime(date))],
                ExecutionOptions {
                    number_format: Some(profile),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![
                Value::Text(fixed.into()),
                Value::Text(general.into()),
                Value::Text(dynamic.into()),
            ]]
        );
    }
}
