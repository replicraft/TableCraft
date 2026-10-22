use tablecraft_core::values::{ExtendedDateTime, LegacyDateTime, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};
fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Events".into(),
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Unused".into(), ScalarType::Text, false).unwrap(),
                Column::new("Stamp".into(), ScalarType::DateTime, true).unwrap(),
                Column::new("Precise".into(), ScalarType::DateTimeExtended, true).unwrap(),
            ],
            vec![
                vec![
                    Value::Int32(1),
                    Value::Text("unused".into()),
                    Value::DateTime(LegacyDateTime::from_ole_days(-1.25).unwrap()),
                    Value::DateTimeExtended(
                        ExtendedDateTime::new(1, 1, 1, 23, 59, 59, 9_999_999).unwrap(),
                    ),
                ],
                vec![
                    Value::Int32(2),
                    Value::Text("unused".into()),
                    Value::DateTime(LegacyDateTime::from_ole_days(45658.5).unwrap()),
                    Value::DateTimeExtended(
                        ExtendedDateTime::new(9999, 12, 31, 23, 59, 59, 9_999_999).unwrap(),
                    ),
                ],
                vec![
                    Value::Int32(3),
                    Value::Text("unused".into()),
                    Value::Null,
                    Value::Null,
                ],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}
fn run(sql: &str, source: &MemorySource) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}
#[test]
fn component_calls_work_in_projection_filters_order_grouping_and_correlations() {
    let source = source();
    assert_eq!(run("SELECT Year(Stamp),Month(Stamp),Day(Stamp),Hour(Stamp),Minute(Stamp),Second(Stamp) FROM Events WHERE Id=1",&source).unwrap(),[vec![1899,12,29,6,0,0].into_iter().map(Value::Int16).collect::<Vec<_>>()]);
    assert_eq!(
        run(
            "SELECT Id FROM Events WHERE Year(Stamp)>1900 ORDER BY Hour(Stamp)",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(2)]]
    );
    assert_eq!(
        run(
            "SELECT Year(Max(Stamp)), Sum(Hour(Stamp)) FROM Events",
            &source
        )
        .unwrap(),
        [vec![Value::Int16(2025), Value::Int64(18)]]
    );
    assert_eq!(
        run(
            "SELECT Id,Year(Max(Stamp)) FROM Events GROUP BY Id HAVING Year(Max(Stamp))=1899",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(1), Value::Int16(1899)]]
    );
    assert_eq!(run("SELECT A.Id FROM Events AS A WHERE (SELECT Hour(B.Stamp) FROM Events AS B WHERE B.Id=A.Id)=6",&source).unwrap(),[vec![Value::Int32(1)]]);
    assert_eq!(
        run(
            "SELECT Year(Precise),Second(Precise) FROM Events ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int16(1), Value::Int16(59)],
            vec![Value::Int16(9999), Value::Int16(59)],
            vec![Value::Null, Value::Null]
        ]
    );
}
#[test]
fn component_calls_preserve_errors_and_reject_invalid_shapes_and_grouping() {
    let source = source();
    assert_eq!(
        run("SELECT Year(Stamp),Count(*) FROM Events", &source),
        Err(QueryError::Grouping)
    );
    assert_eq!(
        run("SELECT Year(1) FROM Events", &source).unwrap(),
        vec![vec![Value::Int16(1899)]; 3]
    );
    assert_eq!(
        run("SELECT Id FROM Events WHERE Id>1 AND Year(1/0)=1", &source),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    for sql in [
        "SELECT Year(*) FROM Events",
        "SELECT Month(Stamp,1) FROM Events",
        "SELECT VBA.Hour(Stamp) FROM Events",
        "SELECT Sum(Year(Max(Stamp))) FROM Events",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}

#[test]
fn date_add_retains_all_inputs_aggregates_and_correlations() {
    let source = source();
    assert_eq!(
        run(
            "SELECT Year(DateAdd('yyyy',Id,Stamp)) FROM Events ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int16(1900)],
            vec![Value::Int16(2027)],
            vec![Value::Null]
        ]
    );
    assert_eq!(run("SELECT Id FROM Events WHERE DateAdd('d',Id,Stamp)>#1900-01-01# ORDER BY DateAdd('d',Id,Stamp)",&source).unwrap(),[vec![Value::Int32(2)]]);
    assert_eq!(
        run(
            "SELECT Year(DateAdd('yyyy',Count(*),Max(Stamp))) FROM Events",
            &source
        )
        .unwrap(),
        [vec![Value::Int16(2028)]]
    );
    assert_eq!(run("SELECT A.Id FROM Events AS A WHERE (SELECT Year(DateAdd('yyyy',A.Id,B.Stamp)) FROM Events AS B WHERE B.Id=A.Id)=1900",&source).unwrap(),[vec![Value::Int32(1)]]);
    assert_eq!(
        run("SELECT DateAdd('d',Id,Max(Stamp)) FROM Events", &source),
        Err(QueryError::Grouping)
    );
    assert_eq!(
        run("SELECT DateAdd('d',1/0,Null) FROM Events", &source),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    for sql in [
        "SELECT DateAdd('d',1) FROM Events",
        "SELECT DateAdd('d',1,) FROM Events",
        "SELECT DateAdd(*) FROM Events",
        "SELECT VBA.DateAdd('d',1,Stamp) FROM Events",
        "SELECT Max(DateAdd('d',Count(*),Stamp)) FROM Events",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}

#[test]
fn date_diff_retains_dynamic_arguments_week_options_and_correlations() {
    let source = source();
    assert_eq!(
        run(
            "SELECT DateDiff('d',Stamp,DateAdd('d',Id,Stamp)) FROM Events ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Null]
        ]
    );
    assert_eq!(
        run(
            "SELECT DateDiff('yyyy',Min(Stamp),Max(Stamp)) FROM Events",
            &source
        )
        .unwrap(),
        [vec![Value::Int32(126)]]
    );
    assert_eq!(run("SELECT A.Id FROM Events AS A WHERE (SELECT DateDiff('d',B.Stamp,DateAdd('d',A.Id,B.Stamp)) FROM Events AS B WHERE B.Id=A.Id)=2",&source).unwrap(),[vec![Value::Int32(2)]]);
    assert_eq!(run("SELECT DateDiff('ww',#2026-01-04#,#2026-01-05#,,2),DateDiff('ww',#2026-01-04#,#2026-01-05#,2,3)",&source).unwrap(),[vec![Value::Int32(0),Value::Int32(1)]]);
    assert_eq!(
        run(
            "SELECT DateDiff('ww',Min(Stamp),Max(Stamp),Id) FROM Events",
            &source
        ),
        Err(QueryError::Grouping)
    );
    assert_eq!(
        run("SELECT DateDiff('d',Null,Stamp,1/0) FROM Events", &source),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    assert_eq!(
        run("SELECT DateDiff('d',Null,Stamp,Null) FROM Events", &source),
        Err(QueryError::Evaluation(EvalError::TypeMismatch))
    );
    for sql in [
        "SELECT DateDiff('d',,Stamp) FROM Events",
        "SELECT DateDiff(*) FROM Events",
        "SELECT DateDiff('d',Stamp,Stamp,1,1,1) FROM Events",
        "SELECT VBA.DateDiff('d',Stamp,Stamp) FROM Events",
        "SELECT Sum(DateDiff('d',Min(Stamp),Stamp)) FROM Events",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}

#[test]
fn date_part_retains_dynamic_options_aggregates_and_correlations() {
    let source = source();
    assert_eq!(
        run(
            "SELECT DatePart('w',Stamp,Id) FROM Events ORDER BY Id",
            &source
        )
        .unwrap(),
        [
            vec![Value::Int16(6)],
            vec![Value::Int16(3)],
            vec![Value::Null]
        ]
    );
    assert_eq!(
        run(
            "SELECT DatePart('w',#2026-01-01#,Count(*)) FROM Events",
            &source
        )
        .unwrap(),
        [vec![Value::Int16(3)]]
    );
    assert_eq!(
        run("SELECT DatePart('y',Max(Stamp)) FROM Events", &source).unwrap(),
        [vec![Value::Int16(1)]]
    );
    assert_eq!(run("SELECT A.Id FROM Events AS A WHERE (SELECT DatePart('w',B.Stamp,A.Id) FROM Events AS B WHERE B.Id=A.Id)=3",&source).unwrap(),[vec![Value::Int32(2)]]);
    assert_eq!(
        run("SELECT DatePart('ww',Min(Stamp),Id) FROM Events", &source),
        Err(QueryError::Grouping)
    );
    assert_eq!(
        run("SELECT DatePart('ww',#2026-01-01#,,3)", &source).unwrap(),
        [vec![Value::Int16(52)]]
    );
    assert_eq!(
        run("SELECT DatePart('d',Null,1/0) FROM Events", &source),
        Err(QueryError::Evaluation(EvalError::DivisionByZero))
    );
    for sql in [
        "SELECT DatePart('d',) FROM Events",
        "SELECT DatePart(*) FROM Events",
        "SELECT DatePart('d',Stamp,1,1,1) FROM Events",
        "SELECT VBA.DatePart('d',Stamp) FROM Events",
        "SELECT Sum(DatePart('d',Max(Stamp))) FROM Events",
    ] {
        assert_eq!(run(sql, &source), Err(QueryError::Unsupported));
    }
}
