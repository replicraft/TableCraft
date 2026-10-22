use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    functions::dates::{
        FirstDayOfWeek, FirstWeekOfYear, TwoDigitYearWindow,
        parsing::{DateTextFormat, ReferenceYear},
    },
};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::parse_query;

#[test]
fn date_formats_keep_group_identity_aggregate_dependencies_and_correlations() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT Len(Format(A.D,'yyyy-mm-dd')),Count(*) FROM (SELECT #1993-01-27# AS D UNION ALL SELECT #2000-01-01#) AS A GROUP BY Len(Format(A.D,'yyyy-mm-dd'))",
            vec![Value::Int32(10), Value::Int64(2)],
        ),
        (
            "SELECT (SELECT Format(A.D,'yyyy-mm-dd')) FROM (SELECT #1993-01-27# AS D) AS A",
            vec![Value::Text("1993-01-27".into())],
        ),
        (
            "SELECT Format(Max(A.D),'yyyy-mm-dd') FROM (SELECT #1993-01-27# AS D UNION ALL SELECT #2000-01-01#) AS A",
            vec![Value::Text("2000-01-01".into())],
        ),
    ] {
        let parsed = parse_query(sql, Default::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .unwrap();
        assert_eq!(result.rows(), [expected]);
    }
    let parsed = parse_query("SELECT Len(Format(A.D,'yyyy-mm-dd')) FROM (SELECT #1993-01-27# AS D) AS A GROUP BY Len(Format(A.D,'dd-mm-yyyy'))", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source).err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn date_mask_parameters_regional_and_system_week_policies_do_not_leak() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "PARAMETERS P TEXT,M TEXT; SELECT Format(P,M)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (profile, expected) in [
        (Some(DateTextFormat::EnUs), Some("January")),
        (Some(DateTextFormat::DeDe), Some("Januar")),
        (None, None),
        (Some(DateTextFormat::Iso), Some("January")),
    ] {
        let result = plan.execute(
            &source,
            &[
                ("P", Value::Text("1993-01-27".into())),
                ("M", Value::Text("mmmm".into())),
            ],
            ExecutionOptions {
                date_format: profile,
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Text(expected.into())]]);
        } else {
            assert_eq!(result.err(), Some(EvalError::DateFormatRequired.into()));
        }
    }
    let parsed = parse_query(
        "PARAMETERS P TEXT; SELECT Format(P,'yyyy-mm-dd')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (text, reference, window, expected) in [
        ("Feb 29", Some(2024), None, Ok("2024-02-29")),
        ("Feb 29", None, None, Err(EvalError::ReferenceYearRequired)),
        ("1/2/49", None, Some(2049), Ok("2049-01-02")),
        ("1/2/49", None, Some(1999), Ok("1949-01-02")),
    ] {
        let result = plan.execute(
            &source,
            &[("P", Value::Text(text.into()))],
            ExecutionOptions {
                date_format: Some(DateTextFormat::EnUs),
                reference_year: reference.map(|year| ReferenceYear::new(year).unwrap()),
                year_window: window.map(|year| TwoDigitYearWindow::new(year).unwrap()),
                ..Default::default()
            },
        );
        match expected {
            Ok(text) => assert_eq!(result.unwrap().rows(), [vec![Value::Text(text.into())]]),
            Err(error) => assert_eq!(result.err(), Some(QueryError::Evaluation(error))),
        }
    }

    let parsed = parse_query("SELECT Format(#2018-12-31#,'w ww',0,0)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (day, week, expected) in [
        (Some(2), Some(2), Some("1 53")),
        (Some(1), Some(1), Some("2 53")),
        (None, None, None),
    ] {
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                first_day_of_week: day.map(|value| FirstDayOfWeek::new(value).unwrap()),
                first_week_of_year: week.map(|value| FirstWeekOfYear::new(value).unwrap()),
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(result.unwrap().rows(), [vec![Value::Text(expected.into())]]);
        } else {
            assert_eq!(result.err(), Some(EvalError::CalendarPolicyRequired.into()));
        }
    }
}

#[test]
fn date_format_cancellation_and_eager_argument_errors_propagate() {
    let source = MemorySource::default();
    let parsed = parse_query(
        "SELECT Format('1993-01-27 17:04:23','dddd, mmmm d yyyy hh:mm:ss AM/PM')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    let count = || {
        checkpoints.set(checkpoints.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            date_format: Some(DateTextFormat::EnUs),
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
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
                date_format: Some(DateTextFormat::EnUs),
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
    let parsed = parse_query("SELECT Format(Null,'yyyy',1/0)", Default::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source)
            .unwrap()
            .execute(&source, &[], Default::default())
            .err(),
        Some(EvalError::DivisionByZero.into())
    );
}
