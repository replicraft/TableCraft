//! Typed VALUES inputs retain written order and original field presence.
use super::{Finished, Target, query};
use crate::{
    InsertError, MutationOptions, QueryError, plan_insert, plan_insert_with_omissions,
    plan_insert_with_omissions_and_options, plan_insert_with_options, source::TextCollationId,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::InsertValue;
use tablecraft_expressions::EvalError;

fn query_budget() -> ResourceBudget {
    ResourceBudget::new(4 * 1024 * 1024, 100, 1_000_000)
}

#[test]
fn reordered_typed_inputs_are_literal_and_finish_one_complete_schema_ordered_row() {
    let parsed = query(
        "PARAMETERS [Active] BIT, [Caption] TEXT(100), [ID] LONG; INSERT INTO Items(Flag,Label,ID) VALUES([Active],[Caption],[ID]+1)",
    );
    let parameters = [
        ("active", Value::Boolean(true)),
        ("Caption", Value::Text("'); DELETE FROM Items; --".into())),
        ("ID", Value::Int32(7)),
    ];
    let calls = Cell::new(0);
    let result = plan_insert_with_options(
        Target::new(&calls, false),
        parsed.query(),
        &mut query_budget(),
        &mut query_budget(),
        MutationOptions {
            parameters: Some(&parameters),
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();
    let Finished::Complete(values) = result else {
        panic!("the complete entrypoint must keep its backend finish");
    };
    assert_eq!(
        values,
        [
            Value::Int32(8),
            Value::Text("'); DELETE FROM Items; --".into()),
            Value::Boolean(true),
        ]
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn all_declarations_validate_even_unused_before_either_consuming_finish() {
    let parsed =
        query("PARAMETERS [Unused] LONG; INSERT INTO Items(ID,Label,Flag) VALUES(7,NULL,FALSE)");
    for parameters in [
        vec![],
        vec![("different", Value::Int32(1))],
        vec![("Unused", Value::Int32(1)), ("extra", Value::Int32(2))],
        vec![("Unused", Value::Text("1".into()))],
        vec![("Unused", Value::Int16(1))],
    ] {
        for omitted in [false, true] {
            let calls = Cell::new(0);
            let result = if omitted {
                plan_insert_with_omissions_and_options(
                    Target::new(&calls, false),
                    parsed.query(),
                    &mut query_budget(),
                    &mut query_budget(),
                    MutationOptions {
                        parameters: Some(&parameters),
                        ..Default::default()
                    },
                    &|| false,
                )
            } else {
                plan_insert_with_options(
                    Target::new(&calls, false),
                    parsed.query(),
                    &mut query_budget(),
                    &mut query_budget(),
                    MutationOptions {
                        parameters: Some(&parameters),
                        ..Default::default()
                    },
                    &|| false,
                )
            };
            assert!(matches!(
                result,
                Err(InsertError::Query(QueryError::Parameter))
            ));
            assert_eq!(calls.get(), 0);
        }
    }

    for (sql, parameters) in [
        (
            "PARAMETERS [First] LONG, [Second] LONG; INSERT INTO Items(ID,Label,Flag) VALUES(7,NULL,FALSE)",
            vec![("First", Value::Int32(1)), ("FIRST", Value::Int32(2))],
        ),
        (
            "PARAMETERS [Unused] TEXT(2); INSERT INTO Items(ID,Label,Flag) VALUES(7,NULL,FALSE)",
            vec![("Unused", Value::Text("abc".into()))],
        ),
    ] {
        let calls = Cell::new(0);
        let parsed = query(sql);
        assert!(matches!(
            plan_insert_with_options(
                Target::new(&calls, false),
                parsed.query(),
                &mut query_budget(),
                &mut query_budget(),
                MutationOptions {
                    parameters: Some(&parameters),
                    ..Default::default()
                },
                &|| false,
            ),
            Err(InsertError::Query(QueryError::Parameter))
        ));
        assert_eq!(calls.get(), 0);
    }

    for omission_entrypoint in [false, true] {
        let calls = Cell::new(0);
        let result = if omission_entrypoint {
            plan_insert_with_omissions(
                Target::new(&calls, false),
                parsed.query(),
                &mut query_budget(),
                &|| false,
            )
        } else {
            plan_insert(
                Target::new(&calls, false),
                parsed.query(),
                &mut query_budget(),
                &|| false,
            )
        };
        assert!(matches!(
            result,
            Err(InsertError::Query(QueryError::Unsupported))
        ));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn typed_null_is_supplied_and_omitted_generated_companions_are_never_guessed() {
    let parsed = query("PARAMETERS [Caption] TEXT; INSERT INTO Items(Label) VALUES([Caption])");
    let parameters = [("Caption", Value::Null)];
    for companions in [false, true] {
        let calls = Cell::new(0);
        let result = plan_insert_with_omissions_and_options(
            Target::new(&calls, companions),
            parsed.query(),
            &mut query_budget(),
            &mut query_budget(),
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
        let Finished::Presence(values) = result else {
            panic!("omissions must reach the presence-bearing backend");
        };
        let label = usize::from(!companions);
        assert_eq!(values.len(), if companions { 2 } else { 3 });
        assert!(matches!(&values[label], InsertValue::Supplied(Value::Null)));
        for (position, value) in values.iter().enumerate() {
            if position != label {
                assert!(matches!(value, InsertValue::Omitted));
            }
        }
        assert_eq!(calls.get(), 1);
    }

    for sql in [
        "PARAMETERS [Caption] TEXT; INSERT INTO Items VALUES([Caption],FALSE)",
        "PARAMETERS [Caption] TEXT; INSERT INTO Items(ID) VALUES([Caption])",
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        assert!(
            plan_insert_with_omissions_and_options(
                Target::new(&calls, true),
                parsed.query(),
                &mut query_budget(),
                &mut query_budget(),
                MutationOptions {
                    parameters: Some(&parameters),
                    ..Default::default()
                },
                &|| false,
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn a_late_values_error_never_sends_a_prefix_or_spends_the_backend_allowance() {
    let parsed = query(
        "PARAMETERS [ID] LONG, [Divisor] LONG; INSERT INTO Items(ID,Label,Flag) VALUES([ID],1/([Divisor]-[ID]),TRUE)",
    );
    let parameters = [("ID", Value::Int32(7)), ("Divisor", Value::Int32(7))];
    let calls = Cell::new(0);
    let mut native = query_budget();
    let before = native.remaining(Resource::Steps);
    assert!(matches!(
        plan_insert_with_options(
            Target::new(&calls, false),
            parsed.query(),
            &mut query_budget(),
            &mut native,
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        ),
        Err(InsertError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(native.remaining(Resource::Steps), before);
    assert_eq!(calls.get(), 0);
}

#[test]
fn independent_query_backend_limits_and_terminal_cancellation_keep_one_finish() {
    let parsed =
        query("PARAMETERS [ID] LONG; INSERT INTO Items(ID,Label,Flag) VALUES([ID],NULL,FALSE)");
    let parameters = [("ID", Value::Int32(7))];
    for native_steps in [6, 7] {
        let calls = Cell::new(0);
        let mut editor = Target::new(&calls, false);
        editor.backend_steps = 7;
        let mut native = ResourceBudget::new(0, 0, native_steps);
        let result = plan_insert_with_options(
            editor,
            parsed.query(),
            &mut query_budget(),
            &mut native,
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        );
        if native_steps == 7 {
            assert!(result.is_ok());
            assert_eq!(native.remaining(Resource::Steps), 0);
            assert_eq!(calls.get(), 1);
        } else {
            assert!(matches!(result, Err(InsertError::Backend(_))));
            assert_eq!(calls.get(), 0);
        }
    }

    let calls = Cell::new(0);
    let mut native = ResourceBudget::new(0, 0, 7);
    assert!(matches!(
        plan_insert_with_options(
            Target::new(&calls, false),
            parsed.query(),
            &mut ResourceBudget::new(0, 100, 1_000_000),
            &mut native,
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        ),
        Err(InsertError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(native.remaining(Resource::Steps), 7);
    assert_eq!(calls.get(), 0);

    let polls = Cell::new(0);
    let calls = Cell::new(0);
    plan_insert_with_options(
        Target::new(&calls, false),
        parsed.query(),
        &mut query_budget(),
        &mut query_budget(),
        MutationOptions {
            parameters: Some(&parameters),
            ..Default::default()
        },
        &|| {
            polls.set(polls.get() + 1);
            false
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 1);
    let terminal = polls.get();
    polls.set(0);
    calls.set(0);
    let mut native = ResourceBudget::new(0, 0, 7);
    assert!(matches!(
        plan_insert_with_options(
            Target::new(&calls, false),
            parsed.query(),
            &mut query_budget(),
            &mut native,
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            },
        ),
        Err(InsertError::Query(QueryError::Cancelled))
    ));
    assert_eq!(native.remaining(Resource::Steps), 7);
    assert_eq!(calls.get(), 0);
}

#[test]
fn values_do_not_gain_source_fields_hidden_providers_or_destination_policy_lineage() {
    let parameters = [("ID", Value::Int32(7))];
    for sql in [
        "PARAMETERS [ID] LONG; INSERT INTO Items(ID,Label,Flag) VALUES(Items.ID,NULL,FALSE)",
        "PARAMETERS [ID] LONG; INSERT INTO Items(ID,Label,Flag) VALUES(Missing,NULL,FALSE)",
        "PARAMETERS [ID] LONG; INSERT INTO Items(ID,Label,Flag) VALUES(@@IDENTITY,NULL,FALSE)",
        "PARAMETERS [ID] LONG; INSERT INTO Items(ID,Label,Flag) VALUES(Date(),NULL,FALSE)",
        "PARAMETERS [ID] LONG; INSERT INTO Items(ID,Label,Flag) VALUES(DCount('*','Hidden'),NULL,FALSE)",
    ] {
        let parsed = query(sql);
        let calls = Cell::new(0);
        assert!(
            plan_insert_with_options(
                Target::new(&calls, false),
                parsed.query(),
                &mut query_budget(),
                &mut query_budget(),
                MutationOptions {
                    parameters: Some(&parameters),
                    ..Default::default()
                },
                &|| false,
            )
            .is_err()
        );
        assert_eq!(calls.get(), 0);
    }
    let parsed =
        query("PARAMETERS [ID] LONG; INSERT INTO Items(ID,Label,Flag) VALUES([ID],NULL,FALSE)");
    let calls = Cell::new(0);
    let policies = [None, Some(TextCollationId::from_host(7)), None];
    assert!(
        plan_insert_with_options(
            Target::new(&calls, false),
            parsed.query(),
            &mut query_budget(),
            &mut query_budget(),
            MutationOptions {
                parameters: Some(&parameters),
                text_collations: &policies,
                ..Default::default()
            },
            &|| false,
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
}
