//! Interval calls share read lowering while retaining original-row mutation semantics.
use super::*;
use tablecraft_core::values::LegacyDateTime;
use tablecraft_expressions::EvalError;

#[test]
fn explicit_dates_use_query_interval_rules_and_optional_defaults() {
    for (expression, expected) in [
        ("Year(DateAdd('yyyy',1,#2000-02-29#))", 2001),
        ("Day(DateAdd('m',1,#2024-01-31#))", 29),
        ("DateDiff('d',#2024-02-28#,#2024-03-01#)", 2),
        ("DateDiff('yyyy',#2023-12-31#,#2024-01-01#)", 1),
        ("DatePart('q',#2024-10-01#)", 4),
        ("DatePart('w',#2024-01-01#)", 2),
        ("DatePart('w',#2024-01-01#,2)", 1),
        ("DatePart('ww',#2024-01-07#,,2)", 2),
        ("DateDiff('ww',#2024-01-01#,#2024-01-08#,,2)", 1),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score=CLng({expression}) WHERE Id=1"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false)
            .unwrap_or_else(|error| panic!("{expression}: {error:?}"));
        assert_eq!(
            result.into_backend(),
            [(0, Value::Int32(expected))],
            "{expression}"
        );
        assert_eq!(finishes.get(), 1);
    }

    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "DELETE FROM Items WHERE DateDiff('d',#2024-01-01#,DateAdd('d',Id,#2024-01-01#))=3",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_delete(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.into_backend(), [2]);
    assert_eq!(finishes.get(), 1);
}

#[test]
fn native_date_carriers_keep_original_fields_and_null_results() {
    let (mut editor, finishes) = source(true);
    editor.snapshot = Snapshot::new(
        "Items".into(),
        vec![
            Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
            Column::new("Moment".into(), ScalarType::DateTime, true).unwrap(),
            Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
        ],
        vec![
            vec![
                Value::Int32(1),
                Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap()),
                Value::Null,
            ],
            vec![
                Value::Int32(2),
                Value::DateTime(LegacyDateTime::from_ole_days(3.0).unwrap()),
                Value::Null,
            ],
            vec![Value::Int32(3), Value::Null, Value::Null],
        ],
        SchemaPolicy {
            max_name_bytes: 255,
            max_columns: 256,
            names: NameComparison::AsciiInsensitive,
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let parsed = parse_query(
        "UPDATE Items SET Score=Year(DateAdd('yyyy',Id,Moment))+DatePart('d',Moment)",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(
        result.into_backend(),
        [
            (0, Value::Int32(1902)),
            (1, Value::Int32(1904)),
            (2, Value::Null)
        ]
    );
    assert_eq!(finishes.get(), 1);
}

#[test]
fn explicit_system_settings_and_null_controls_have_typed_failures() {
    for (expression, expected) in [
        (
            "DatePart('ww',#2024-01-01#,0)",
            EvalError::CalendarPolicyRequired,
        ),
        (
            "DateDiff('ww',#2024-01-01#,#2024-01-08#,1,0)",
            EvalError::CalendarPolicyRequired,
        ),
        ("DatePart('w',#2024-01-01#,NULL)", EvalError::TypeMismatch),
        (
            "DateDiff('d',#2024-01-01#,#2024-01-02#,,NULL)",
            EvalError::TypeMismatch,
        ),
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression}"),
            ParseLimits::default(),
        )
        .unwrap();
        let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false);
        assert!(
            matches!(result, Err(UpdateError::Query(QueryError::Evaluation(error))) if error == expected),
            "{expression}"
        );
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn late_invalid_interval_and_eager_operands_never_finish_a_prefix() {
    for sql in [
        "UPDATE Items SET Score=DatePart(IIf(Id<3,'d','invalid'),#2024-01-01#)",
        "UPDATE Items SET Score=Year(DateAdd('yyyy',Id-1,#9999-01-01#))",
        "UPDATE Items SET Score=IIf(TRUE,1,DateDiff('invalid',#2024-01-01#,#2024-01-02#))",
        "UPDATE Items SET Score=DatePart('d',#2024-01-01#,1,1/0)",
        "UPDATE Items SET Score=1 WHERE FALSE AND DatePart(IIf(Id<3,'d','invalid'),#2024-01-01#)>0",
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
        assert_eq!(finishes.get(), 0);
    }

    for expression in [
        "DateAdd('d',1)",
        "DateAdd('d',, #2024-01-01#)",
        "DatePart('d')",
        "DateDiff('d',#2024-01-01#)",
        "DatePart('d',#2024-01-01#,1,1,1)",
        "Other.DateAdd('d',1,#2024-01-01#)",
        "DateAdd('d',1,Now())",
        "DatePart('d',Array(1))",
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
fn interval_evaluation_retains_caller_budgets_and_terminal_cancellation() {
    let parsed = parse_query(
        "UPDATE Items SET Score=DateDiff('d',#2024-01-01#,DateAdd('d',Id,#2024-01-01#))+DatePart('q',#2024-01-01#)",
        ParseLimits::default(),
    )
    .unwrap();
    let (editor, finishes) = source(false);
    let polls = Cell::new(0);
    plan_update(editor, parsed.query(), &mut budget(), None, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(finishes.get(), 1);
    let terminal = polls.get();

    let (editor, finishes) = source(false);
    let polls = Cell::new(0);
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| {
        polls.set(polls.get() + 1);
        polls.get() == terminal
    });
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
        assert!(plan_update(editor, parsed.query(), &mut allowance, None, &|| false).is_err());
        assert_eq!(finishes.get(), 0);
    }
}
