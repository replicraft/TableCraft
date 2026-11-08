//! Explicit DELETE inputs and policies use the shared mutation context.
use super::{DeleteError, budget, plan_delete, snapshot_source, source};
use crate::{
    MutationOptions, QueryError, plan_delete_with_options,
    source::{CollationResolver, TextCollationId},
};
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::SnapshotError;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

const FIELD_FOLD: TextCollationId = TextCollationId::from_host(7);

#[test]
fn typed_parameters_select_original_handles_before_one_backend_finish() {
    let query = parse_query(
        "PARAMETERS [lower] LONG, [upper] LONG; DELETE i.* FROM Items AS i WHERE i.Id BETWEEN [lower] AND [upper]",
        ParseLimits::default(),
    )
    .unwrap();
    for (lower, expected) in [(Value::Int32(2), vec![1, 2]), (Value::Null, vec![])] {
        let parameters = [("LOWER", lower), ("upper", Value::Int32(3))];
        let (editor, calls) = source(false);
        let plan = plan_delete_with_options(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), expected.len());
        assert_eq!(plan.backend(), &expected);
        assert_eq!(plan.into_backend(), expected);
        assert_eq!(calls.get(), 1);
    }

    let (editor, calls) = source(false);
    assert!(matches!(
        plan_delete(editor, query.query(), &mut budget(), None, &|| false),
        Err(DeleteError::Query(QueryError::Unsupported))
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn missing_bad_extra_and_unused_declared_inputs_refuse_before_empty_finish() {
    let query = parse_query(
        "PARAMETERS chosen LONG; DELETE FROM Items WHERE FALSE",
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
        let (editor, calls) = source(true);
        assert!(matches!(
            plan_delete_with_options(
                editor,
                query.query(),
                &mut budget(),
                &mut budget(),
                MutationOptions {
                    parameters: Some(&parameters),
                    ..Default::default()
                },
                &|| false,
            ),
            Err(DeleteError::Query(QueryError::Parameter))
        ));
        assert_eq!(calls.get(), 0);
    }

    for (sql, parameters) in [
        (
            "PARAMETERS chosen LONG, other LONG; DELETE FROM Items WHERE FALSE",
            vec![("chosen", Value::Int32(1)), ("CHOSEN", Value::Int32(2))],
        ),
        (
            "PARAMETERS unused TEXT(2); DELETE FROM Items WHERE FALSE",
            vec![("unused", Value::Text("abc".into()))],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let (editor, calls) = source(true);
        assert!(matches!(
            plan_delete_with_options(
                editor,
                query.query(),
                &mut budget(),
                &mut budget(),
                MutationOptions {
                    parameters: Some(&parameters),
                    ..Default::default()
                },
                &|| false,
            ),
            Err(DeleteError::Query(QueryError::Parameter))
        ));
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn typed_parameters_cannot_hide_a_late_eager_predicate_failure() {
    let query = parse_query(
        "PARAMETERS divisor LONG, inert TEXT; DELETE FROM Items WHERE Id>0 AND 120/(divisor-Id)>0",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [
        ("divisor", Value::Int32(3)),
        ("inert", Value::Text("DELETE FROM Items".into())),
    ];
    let (editor, calls) = source(false);
    let mut native = budget();
    let before = native.remaining(Resource::Steps);
    assert!(matches!(
        plan_delete_with_options(
            editor,
            query.query(),
            &mut budget(),
            &mut native,
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        ),
        Err(DeleteError::Query(QueryError::Evaluation(_)))
    ));
    assert_eq!(native.remaining(Resource::Steps), before);
    assert_eq!(calls.get(), 0);
}

#[test]
fn query_and_backend_limits_are_independent_and_cancellation_precedes_finish() {
    let query = parse_query(
        "PARAMETERS chosen LONG; DELETE FROM Items WHERE Id>=chosen",
        ParseLimits::default(),
    )
    .unwrap();
    let parameters = [("chosen", Value::Int32(2))];
    let (mut editor, calls) = source(false);
    editor.backend_steps = 7;
    let mut query_pool = budget();
    let query_available = query_pool.remaining(Resource::Steps);
    let mut native_pool = ResourceBudget::new(0, 0, 6);
    assert!(matches!(
        plan_delete_with_options(
            editor,
            query.query(),
            &mut query_pool,
            &mut native_pool,
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        ),
        Err(DeleteError::Backend(SnapshotError::Limit(_)))
    ));
    assert!(query_pool.remaining(Resource::Steps) < query_available);
    assert_eq!(calls.get(), 0);

    let (mut editor, calls) = source(false);
    editor.backend_steps = 7;
    let mut native_pool = ResourceBudget::new(0, 0, 7);
    assert!(matches!(
        plan_delete_with_options(
            editor,
            query.query(),
            &mut ResourceBudget::new(0, 100, 100_000),
            &mut native_pool,
            MutationOptions {
                parameters: Some(&parameters),
                ..Default::default()
            },
            &|| false,
        ),
        Err(DeleteError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(native_pool.remaining(Resource::Steps), 7);
    assert_eq!(calls.get(), 0);

    let (mut editor, calls) = source(false);
    editor.backend_steps = 7;
    let mut native_pool = ResourceBudget::new(0, 0, 7);
    let polls = Cell::new(0);
    let result = plan_delete_with_options(
        editor,
        query.query(),
        &mut budget(),
        &mut native_pool,
        MutationOptions {
            parameters: Some(&parameters),
            ..Default::default()
        },
        &|| {
            polls.set(polls.get() + 1);
            polls.get() >= 7
        },
    );
    assert!(matches!(
        result,
        Err(DeleteError::Query(QueryError::Cancelled))
    ));
    assert_eq!(native_pool.remaining(Resource::Steps), 7);
    assert_eq!(calls.get(), 0);

    let (mut editor, calls) = source(false);
    editor.backend_steps = 7;
    let mut native_pool = ResourceBudget::new(0, 0, 7);
    let plan = plan_delete_with_options(
        editor,
        query.query(),
        &mut budget(),
        &mut native_pool,
        MutationOptions {
            parameters: Some(&parameters),
            ..Default::default()
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(plan.into_backend(), [1, 2]);
    assert_eq!(native_pool.remaining(Resource::Steps), 0);
    assert_eq!(calls.get(), 1);
}

struct Comparer(bool);

impl Collation for Comparer {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        if self.0 {
            Ok(left
                .bytes()
                .map(|byte| byte.to_ascii_lowercase())
                .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase())))
        } else {
            Ok(left.cmp(right))
        }
    }
}

static DEFAULT_COMPARER: Comparer = Comparer(false);
static FIELD_COMPARER: Comparer = Comparer(true);

struct Resolver(Cell<usize>);

impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.0.set(self.0.get() + 1);
        if policy == FIELD_FOLD {
            Ok(&FIELD_COMPARER)
        } else {
            Err(EvalError::CollationRequired.into())
        }
    }
}

#[test]
fn field_policy_overrides_default_for_typed_text_parameter_in_both_orders() {
    for predicate in ["Name=needle", "needle=Name"] {
        let query = parse_query(
            &format!("PARAMETERS needle TEXT; DELETE FROM Items WHERE {predicate}"),
            ParseLimits::default(),
        )
        .unwrap();
        let parameters = [("needle", Value::Text("ALPHA".into()))];
        let (editor, calls) = snapshot_source(
            vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Name".into(), ScalarType::Text, false).unwrap(),
            ],
            vec![
                vec![Value::Int32(1), Value::Text("Alpha".into())],
                vec![Value::Int32(2), Value::Text("Beta".into())],
            ],
        );
        let resolver = Resolver(Cell::new(0));
        let policies = [None, Some(FIELD_FOLD)];
        let plan = plan_delete_with_options(
            editor,
            query.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                parameters: Some(&parameters),
                collation: Some(&DEFAULT_COMPARER),
                collation_resolver: Some(&resolver),
                text_collations: &policies,
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.into_backend(), [0]);
        assert_eq!(calls.get(), 1);
        assert!(resolver.0.get() > 0);
    }
}
