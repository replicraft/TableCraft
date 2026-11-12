//! Named date conversions use explicit evaluator context without runtime authority.
use super::*;
use tablecraft_expressions::{EvalError, functions::dates::parsing::DateTextFormat};
use tablecraft_query::{MutationOptions, plan_batch_update_with_options};

#[test]
fn named_date_conversions_accept_explicit_dates_and_numeric_carriers() {
    for (expression, expected) in [
        ("Year(CDate(#2024-02-29#))", 2024),
        ("Day(DateValue(#2024-02-29 12:34:56#))", 29),
        ("Hour(TimeValue(#2024-02-29 12:34:56#))", 12),
        ("Year(CDate(2.5))", 1900),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression} WHERE Id=1"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
        assert_eq!(result.into_backend(), [(0, Value::Int32(expected))]);
        assert_eq!(finishes.get(), 1);
    }
}

#[test]
fn text_dates_use_explicit_profiles_and_missing_context_is_typed() {
    for (format, expression, expected) in [
        (DateTextFormat::EnUs, "Day(CDate('12/31/2026'))", 31),
        (DateTextFormat::DeDe, "Day(DateValue('31.12.2026'))", 31),
        (DateTextFormat::Iso, "Hour(TimeValue('23:04:05'))", 23),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression} WHERE Id=1"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_batch_update_with_options(
            editor,
            parsed.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                date_format: Some(format),
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(result.into_backend(), [(0, Value::Int32(expected))]);
        assert_eq!(finishes.get(), 1);
    }

    for (expression, format, expected) in [
        (
            "Day(CDate('12/31/2026'))",
            None,
            EvalError::DateFormatRequired,
        ),
        (
            "Day(DateValue('1/2'))",
            Some(DateTextFormat::EnUs),
            EvalError::ReferenceYearRequired,
        ),
        (
            "Day(CDate(NULL))",
            Some(DateTextFormat::Iso),
            EvalError::InvalidNullConversion,
        ),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression}"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_batch_update_with_options(
            editor,
            parsed.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                date_format: format,
                ..Default::default()
            },
            &|| false,
        );
        assert!(
            matches!(result, Err(UpdateError::Query(QueryError::Evaluation(error))) if error == expected),
            "{expression}"
        );
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn late_conversion_failure_and_forbidden_nested_authority_have_no_backend_prefix() {
    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "UPDATE Items SET Score=Year(CDate(IIf(Id<3,2.5,99999999)))",
        ParseLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
        Err(UpdateError::Query(QueryError::Evaluation(_)))
    ));
    assert_eq!(finishes.get(), 0);

    for expression in [
        "CDate()",
        "DateValue(1,2)",
        "TimeValue(1,2)",
        "CVDate(1)",
        "Other.CDate(1)",
        "CDate(Now())",
        "CDate(Array(1))",
        "CDate(DCount('*','Hidden'))",
        "CDate(UserFunction(Score))",
        "CDate(Label LIKE 'A*')",
    ] {
        let (editor, finishes) = source(true);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression} WHERE FALSE"),
            ParseLimits::default(),
        )
        .unwrap();
        assert!(
            matches!(
                plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
                Err(UpdateError::Query(QueryError::Unsupported))
            ),
            "{expression}"
        );
        assert_eq!(finishes.get(), 0);
    }
}
