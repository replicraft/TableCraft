//! Pure query-callable math uses original rows and one complete backend finish.
use super::*;
use tablecraft_expressions::{
    EvalError,
    functions::{conversions::text::NumberFormat, dates::parsing::DateTextFormat},
};
use tablecraft_query::{MutationOptions, plan_batch_update_with_options};

#[test]
fn all_pure_math_calls_reuse_checked_evaluation_and_explicit_integral_conversion() {
    for (expression, expected) in [
        ("Sgn(-7)", -1),
        ("Sqr(81)", 9),
        ("Exp(0)", 1),
        ("Log(1)", 0),
        ("Sin(0)", 0),
        ("Cos(0)", 1),
        ("Tan(0)", 0),
        ("Atn(0)", 0),
        ("RGB(1,2,3)", 197121),
        ("QBColor(1)", 8388608),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score=CLng({expression}) WHERE Id=1"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false)
            .unwrap_or_else(|error| panic!("{expression}: {error:?}"));
        assert_eq!(result.into_backend(), [(0, Value::Int32(expected))]);
        assert_eq!(finishes.get(), 1);
    }

    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "DELETE FROM Items WHERE CLng(Sqr(Id))=2",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_delete(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.into_backend(), [2, 3]);
    assert_eq!(finishes.get(), 1);
}

#[test]
fn domain_overflow_and_unused_operands_never_publish_an_earlier_row() {
    for sql in [
        "UPDATE Items SET Score=CLng(Sqr(2-Id))",
        "UPDATE Items SET Score=CLng(Exp(Id*11))",
        "UPDATE Items SET Score=IIf(TRUE,1,CLng(Log(0)))",
        "UPDATE Items SET Score=Nz(Id,CLng(Sqr(-1)))",
        "UPDATE Items SET Score=1 WHERE FALSE AND Sqr(2-Id)>0",
        "UPDATE Items SET Score=RGB(NULL,1/0,0)",
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert!(
            matches!(
                plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
                Err(UpdateError::Query(QueryError::Evaluation(_)))
            ),
            "{sql}"
        );
        assert_eq!(finishes.get(), 0, "{sql}");
    }
}

#[test]
fn math_nulls_propagate_but_arity_and_runtime_authority_stay_closed() {
    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "UPDATE Items SET Score=CLng(Sqr(Score)) WHERE Id=2",
        ParseLimits::default(),
    )
    .unwrap();
    // CLng(NULL) is an invalid conversion, rather than an invented stored NULL.
    assert!(plan_update(editor, parsed.query(), &mut budget(), None, &|| false).is_err());
    assert_eq!(finishes.get(), 0);

    let (editor, finishes) = source(false);
    let parsed = parse_query("UPDATE Items SET Score=Sqr(NULL)", ParseLimits::default()).unwrap();
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(
        result.into_backend(),
        [
            (0, Value::Null),
            (1, Value::Null),
            (2, Value::Null),
            (3, Value::Null)
        ]
    );
    assert_eq!(finishes.get(), 1);

    for expression in [
        "Sqr()",
        "Exp(1,2)",
        "RGB(1,2)",
        "QBColor(0,1)",
        "Other.Sin(0)",
        "Rnd()",
        "Timer()",
        "Array(1)",
        "Eval('1')",
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

#[test]
fn cstr_requires_only_its_actual_numeric_or_date_context() {
    for (expression, options, expected) in [
        (
            "CStr(1.25)",
            MutationOptions {
                number_format: Some(NumberFormat::EnUs),
                ..Default::default()
            },
            "1.25",
        ),
        (
            "CStr(1.25)",
            MutationOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            },
            "1,25",
        ),
        (
            "CStr(DateSerial(2026,10,20))",
            MutationOptions {
                date_format: Some(DateTextFormat::Iso),
                ..Default::default()
            },
            "2026-10-20",
        ),
        ("CStr(TRUE)", MutationOptions::default(), "True"),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Label={expression} WHERE Id=1"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_batch_update_with_options(
            editor,
            parsed.query(),
            &mut budget(),
            &mut budget(),
            options,
            &|| false,
        )
        .unwrap();
        assert_eq!(result.into_backend(), [(0, Value::Text(expected.into()))]);
        assert_eq!(finishes.get(), 1);
    }

    for (expression, expected) in [
        ("CStr(1.25)", EvalError::NumberFormatRequired),
        (
            "CStr(DateSerial(2026,10,20))",
            EvalError::DateFormatRequired,
        ),
        ("CStr(NULL)", EvalError::InvalidNullConversion),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Label={expression} WHERE Id=1"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_batch_update_with_options(
            editor,
            parsed.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions::default(),
            &|| false,
        );
        assert!(
            matches!(result, Err(UpdateError::Query(QueryError::Evaluation(error))) if error == expected)
        );
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn formatted_math_staging_obeys_cumulative_work_and_terminal_cancellation() {
    let parsed = parse_query(
        "UPDATE Items SET Label=CStr(Sqr(Id*Id))",
        ParseLimits::default(),
    )
    .unwrap();
    let options = || MutationOptions {
        number_format: Some(NumberFormat::Invariant),
        ..Default::default()
    };
    let (editor, finishes) = source(false);
    let polls = Cell::new(0);
    plan_batch_update_with_options(
        editor,
        parsed.query(),
        &mut budget(),
        &mut budget(),
        options(),
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(finishes.get(), 1);
    let terminal = polls.get();

    let (editor, finishes) = source(false);
    let polls = Cell::new(0);
    let result = plan_batch_update_with_options(
        editor,
        parsed.query(),
        &mut budget(),
        &mut budget(),
        options(),
        &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        },
    );
    assert!(matches!(
        result,
        Err(UpdateError::Query(QueryError::Cancelled))
    ));
    assert_eq!(finishes.get(), 0);

    for mut allowance in [
        ResourceBudget::new(0, 100_000, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(16 * 1024 * 1024, 100_000, 0),
    ] {
        let (editor, finishes) = source(false);
        let result = plan_batch_update_with_options(
            editor,
            parsed.query(),
            &mut allowance,
            &mut budget(),
            options(),
            &|| false,
        );
        assert!(result.is_err());
        assert_eq!(finishes.get(), 0);
    }
}
