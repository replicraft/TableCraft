//! Declared action inputs remain typed and complete before backend finish.
use super::{Recorded, budget, create_editor};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_query::{QueryError, UpdateError, UpdateOptions, plan_batch_update_with_comparison};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn typed_parameters_filter_original_rows_and_preserve_null_assignments() {
    let query = parse_query(
        "PARAMETERS chosen LONG, replacement LONG; UPDATE Items SET [Group]=replacement, Score=[Group] WHERE Id=chosen",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [("CHOSEN", Value::Int32(2)), ("replacement", Value::Null)];
    let (editor, calls) = create_editor();
    let result = plan_batch_update_with_comparison(
        editor,
        query.query(),
        &mut budget(),
        &mut budget(),
        UpdateOptions {
            parameters: Some(&parameters),
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();

    assert_eq!(result.matched_rows(), 1);
    let expected: Recorded = vec![
        ("Group".into(), vec![(1, Value::Null)]),
        ("Score".into(), vec![(1, Value::Int32(20))]),
    ];
    assert_eq!(result.into_backend(), expected);
    assert_eq!(calls.get(), 1);
}

#[test]
fn missing_extra_duplicate_and_wrong_carriers_refuse_even_for_unused_parameters() {
    let query = parse_query(
        "PARAMETERS chosen LONG; UPDATE Items SET Score=1 WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    for parameters in [
        vec![],
        vec![("other", Value::Int32(1))],
        vec![("chosen", Value::Int32(1)), ("extra", Value::Int32(2))],
        vec![("chosen", Value::Text("1".into()))],
        vec![("chosen", Value::Int16(1))],
    ] {
        let (editor, calls) = create_editor();
        let result = plan_batch_update_with_comparison(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            UpdateOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        );
        assert!(matches!(
            result,
            Err(UpdateError::Query(QueryError::Parameter))
        ));
        assert_eq!(calls.get(), 0);
    }

    let query = parse_query(
        "PARAMETERS chosen LONG, other LONG; UPDATE Items SET Score=1 WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [("chosen", Value::Int32(1)), ("CHOSEN", Value::Int32(2))];
    let (editor, calls) = create_editor();
    assert!(matches!(
        plan_batch_update_with_comparison(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            UpdateOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        ),
        Err(UpdateError::Query(QueryError::Parameter))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn declaration_constraints_and_legacy_refusal_precede_empty_backend_finish() {
    let query = parse_query(
        "PARAMETERS text_value TEXT(2); UPDATE Items SET Score=1 WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    let inputs = [("text_value", Value::Text("abc".into()))];
    for parameters in [None, Some(inputs.as_slice())] {
        let (editor, calls) = create_editor();
        let result = plan_batch_update_with_comparison(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            UpdateOptions {
                parameters,
                ..Default::default()
            },
            &|| false,
        );
        if parameters.is_some() {
            assert!(matches!(
                result,
                Err(UpdateError::Query(QueryError::Parameter))
            ));
        } else {
            assert!(matches!(
                result,
                Err(UpdateError::Query(QueryError::Unsupported))
            ));
        }
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn parameter_values_cannot_hide_late_rhs_failure_or_become_sql() {
    let query = parse_query(
        "PARAMETERS divisor LONG, inert TEXT; UPDATE Items SET [Group]=Id, Score=120/(divisor-Id)",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [
        ("divisor", Value::Int32(4)),
        ("inert", Value::Text("DELETE FROM Items".into())),
    ];
    let (editor, calls) = create_editor();
    assert!(
        plan_batch_update_with_comparison(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            UpdateOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn sql_looking_text_is_only_a_literal_value() {
    let query = parse_query(
        "PARAMETERS inert TEXT; UPDATE Items SET Score=1 WHERE inert='DELETE FROM Items' AND Id=2",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [("inert", Value::Text("DELETE FROM Items".into()))];
    let (editor, calls) = create_editor();
    let result = plan_batch_update_with_comparison(
        editor,
        query.query(),
        &mut budget(),
        &mut budget(),
        UpdateOptions {
            parameters: Some(&parameters),
            collation: Some(&super::comparison::DEFAULT_COMPARER),
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        result.into_backend(),
        vec![("Score".into(), vec![(1, Value::Int32(1))])]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn fields_take_precedence_over_same_named_declared_parameters() {
    let query = parse_query(
        "PARAMETERS Id LONG; UPDATE Items SET Score=Id WHERE Id=2",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [("Id", Value::Int32(999))];
    let (editor, calls) = create_editor();
    let result = plan_batch_update_with_comparison(
        editor,
        query.query(),
        &mut budget(),
        &mut budget(),
        UpdateOptions {
            parameters: Some(&parameters),
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        result.into_backend(),
        vec![("Score".into(), vec![(1, Value::Int32(2))])]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn value_parameters_keep_the_supplied_carrier_for_arithmetic() {
    let query = parse_query(
        "PARAMETERS increment VALUE; UPDATE Items SET Score=increment+Id WHERE Id=2",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [("increment", Value::Int16(32767))];
    let (editor, calls) = create_editor();
    let result = plan_batch_update_with_comparison(
        editor,
        query.query(),
        &mut budget(),
        &mut budget(),
        UpdateOptions {
            parameters: Some(&parameters),
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(
        result.into_backend(),
        vec![("Score".into(), vec![(1, Value::Int32(32769))])]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn parameter_name_work_and_cancellation_precede_backend_finish() {
    let query = parse_query(
        "PARAMETERS [first] LONG, [second] LONG; UPDATE Items SET Score=1 WHERE FALSE",
        ParseLimits::default(),
    )
    .unwrap();
    // Equal counts force name matching; the very long unmatched name must be
    // charged before comparison rather than traversed outside the allowance.
    let long_name = "x".repeat(1_000_001);
    let parameters = [
        (long_name.as_str(), Value::Int32(1)),
        ("second", Value::Int32(2)),
    ];
    let (editor, calls) = create_editor();
    assert!(matches!(
        plan_batch_update_with_comparison(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            UpdateOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        ),
        Err(UpdateError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(calls.get(), 0);

    let parameters = [("first", Value::Int32(1)), ("second", Value::Int32(2))];
    let polls = Cell::new(0);
    let (editor, calls) = create_editor();
    let mut query_budget = budget();
    let before = query_budget.remaining(Resource::Steps);
    assert!(matches!(
        plan_batch_update_with_comparison(
            editor,
            query.query(),
            &mut query_budget,
            &mut ResourceBudget::new(0, 0, 0),
            UpdateOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| {
                polls.set(polls.get() + 1);
                polls.get() >= 7
            },
        ),
        Err(UpdateError::Query(QueryError::Cancelled))
    ));
    assert!(query_budget.remaining(Resource::Steps) < before);
    assert_eq!(calls.get(), 0);
}
