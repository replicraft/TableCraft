//! Closed Gregorian expressions retain original-row and one-finish semantics.
use super::*;
use tablecraft_core::values::LegacyDateTime;

#[test]
fn date_components_and_relative_constructors_update_original_rows() {
    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "UPDATE Items SET Score=Year(DateSerial(2024,3,0))+Month(DateSerial(2024,3,0))+Day(DateSerial(2024,3,0))+Hour(TimeSerial(25,0,0))+Minute(TimeSerial(0,61,0))+Second(TimeSerial(0,0,61)) WHERE Id=1",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.into_backend(), [(0, Value::Int32(2058))]);
    assert_eq!(finishes.get(), 1);

    let (editor, finishes) = source(false);
    let parsed = parse_query(
        "DELETE FROM Items WHERE Day(DateSerial(2024,2,Id))=2",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_delete(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(result.into_backend(), [1]);
    assert_eq!(finishes.get(), 1);
}

#[test]
fn constructed_dates_keep_exact_stored_carriers() {
    let (mut editor, finishes) = source(false);
    editor.snapshot = Snapshot::new(
        "Items".into(),
        vec![Column::new("Moment".into(), ScalarType::DateTime, true).unwrap()],
        vec![vec![Value::Null]],
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
        "UPDATE Items SET Moment=DateSerial(1900,1,1)+TimeSerial(12,0,0)",
        ParseLimits::default(),
    )
    .unwrap();
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    let expected = Value::DateTime(LegacyDateTime::from_ole_days(2.5).unwrap());
    assert_eq!(result.into_backend(), [(0, expected)]);
    assert_eq!(finishes.get(), 1);
}

#[test]
fn date_nulls_and_eager_errors_do_not_create_partial_backend_work() {
    let (editor, finishes) = source(false);
    let parsed = parse_query("UPDATE Items SET Score=Year(NULL)", ParseLimits::default()).unwrap();
    let result = plan_update(editor, parsed.query(), &mut budget(), None, &|| false).unwrap();
    assert_eq!(
        result.into_backend(),
        (0..4).map(|row| (row, Value::Null)).collect::<Vec<_>>()
    );
    assert_eq!(finishes.get(), 1);

    for expression in [
        "IIf(FALSE,Year(1/0),1)",
        "DateSerial(NULL,1,1/0)",
        "Year('2024-01-01')",
        "DateSerial(32767,1,1)",
    ] {
        let (editor, finishes) = source(false);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression}"),
            ParseLimits::default(),
        )
        .unwrap();
        assert!(plan_update(editor, parsed.query(), &mut budget(), None, &|| false).is_err());
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn malformed_or_runtime_date_calls_refuse_even_without_rows() {
    for expression in [
        "Year()",
        "Year(NULL,NULL)",
        "DateSerial(2024,1)",
        "TimeSerial(0,0,0,0)",
        "Other.Year(NULL)",
        "Now()",
        "DateValue('2024-01-01')",
    ] {
        let (editor, finishes) = source(true);
        let parsed = parse_query(
            &format!("UPDATE Items SET Score={expression}"),
            ParseLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            plan_update(editor, parsed.query(), &mut budget(), None, &|| false),
            Err(UpdateError::Query(QueryError::Unsupported))
        ));
        assert_eq!(finishes.get(), 0);
    }
}

#[test]
fn date_constructor_work_and_terminal_cancellation_precede_backend_finish() {
    let parsed = parse_query(
        "UPDATE Items SET Score=Year(DateSerial(2024,Id,1))",
        ParseLimits::default(),
    )
    .unwrap();
    let resources = [
        tablecraft_core::limits::Resource::Bytes,
        tablecraft_core::limits::Resource::Rows,
        tablecraft_core::limits::Resource::Steps,
    ];
    let limits = [16 * 1024 * 1024, 100_000, 1_000_000];
    let mut work = budget();
    let polls = Cell::new(0);
    let (editor, finishes) = source(false);
    plan_update(editor, parsed.query(), &mut work, None, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(finishes.get(), 1);
    let terminal = polls.get();

    for index in 0..resources.len() {
        let needed = limits[index] - work.remaining(resources[index]);
        if needed == 0 {
            continue;
        }
        for exact in [true, false] {
            let mut allowance = limits;
            allowance[index] = needed - u64::from(!exact);
            let mut work = ResourceBudget::new(allowance[0], allowance[1], allowance[2]);
            let (editor, finishes) = source(false);
            let result = plan_update(editor, parsed.query(), &mut work, None, &|| false);
            assert_eq!(result.is_ok(), exact);
            assert_eq!(finishes.get(), usize::from(exact));
        }
    }

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
}
