use tablecraft_core::values::{LegacyDateTime, Value};
use tablecraft_expressions::{EvalError, functions::dates::parsing::DateTextFormat};
use tablecraft_query::{ExecutionOptions, QueryError, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn legacy_lower_bound_classification_survives_derived_and_eager_composition() {
    let source = MemorySource::default();
    for declared in [false, true] {
        for sql in [
            "SELECT DateAdd('d',-1,#0100-01-01#)",
            "SELECT D FROM (SELECT DateAdd('s',-1,#0100-01-01#) AS D) Q",
            "SELECT IIf(False,DateAdd('yyyy',-1,#0100-01-01#),0)",
        ] {
            let syntax = parse_query(sql, ParseLimits::default()).unwrap();
            let plan = prepare(syntax.query(), &source).unwrap();
            let error = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    },
                )
                .err();
            assert_eq!(
                error,
                Some(QueryError::Evaluation(EvalError::Domain)),
                "{sql}"
            );
        }
    }
}

#[test]
fn prepared_text_sources_reuse_the_same_legacy_calendar_boundary() {
    let source = MemorySource::default();
    let syntax = parse_query(
        "PARAMETERS D TEXT; SELECT DateAdd('d',-1,D)",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let options = ExecutionOptions {
        date_format: Some(DateTextFormat::Iso),
        ..Default::default()
    };
    let invalid = [("D", Value::Text("0100-01-01".into()))];
    assert_eq!(
        plan.execute(&source, &invalid, options).err(),
        Some(QueryError::Evaluation(EvalError::Domain))
    );

    let valid = [("D", Value::Text("0100-01-02".into()))];
    let expected = Value::DateTime(
        LegacyDateTime::from_calendar("0100-01-01T00:00:00".parse().unwrap()).unwrap(),
    );
    assert_eq!(
        plan.execute(&source, &valid, options).unwrap().rows(),
        [vec![expected]]
    );
}

#[test]
fn count_width_upper_range_and_eager_argument_errors_keep_their_boundaries() {
    let source = MemorySource::default();
    for (sql, expected) in [
        ("SELECT DateAdd('s',-2147483649,NULL)", EvalError::Overflow),
        (
            "SELECT DateAdd('s',1,#9999-12-31 23:59:59#)",
            EvalError::Overflow,
        ),
        (
            "SELECT DateAdd('bad',-1,#0100-01-01#)",
            EvalError::InvalidDateInterval,
        ),
        ("SELECT DateAdd('s',-1,1/0)", EvalError::DivisionByZero),
        ("SELECT DateAdd('bad',-1,1/0)", EvalError::DivisionByZero),
    ] {
        let syntax = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .err(),
            Some(QueryError::Evaluation(expected)),
            "{sql}"
        );
    }
}
