//! Criteria wildcard context is separate from assignment scalar admission.
use super::{DEFAULT_COMPARER, FOLD, ORDINAL, Resolver, budget, editor};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, predicates::LikeMode};
use tablecraft_query::{MutationOptions, QueryError, UpdateError, plan_batch_update_with_options};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn criteria_like_uses_explicit_mode_and_original_field_policy() {
    for (mode, pattern, matched) in [
        (LikeMode::Ansi89, "a*", 1),
        (LikeMode::Ansi89, "a%", 0),
        (LikeMode::Ansi92, "a%", 1),
        (LikeMode::Ansi92, "a*", 0),
    ] {
        let (editor, calls) = editor();
        let resolver = Resolver::default();
        let policies = [None, Some(FOLD), Some(ORDINAL), None];
        let parsed = parse_query(
            &format!("UPDATE Items SET Score=77 WHERE Name LIKE '{pattern}'"),
            ParseLimits::default(),
        )
        .unwrap();
        let plan = plan_batch_update_with_options(
            editor,
            parsed.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                like_mode: Some(mode),
                collation: Some(&DEFAULT_COMPARER),
                collation_resolver: Some(&resolver),
                text_collations: &policies,
                ..Default::default()
            },
            &|| false,
        )
        .unwrap();
        assert_eq!(plan.matched_rows(), matched);
        assert_eq!(calls.get(), 1);
        assert!(resolver.calls.get() > 0, "field policy must reach matching");
        if matched == 1 {
            assert_eq!(
                plan.backend(),
                &vec![("Score".into(), vec![(0, Value::Int32(77))])]
            );
        }
    }
}

#[test]
fn criteria_like_retains_mode_errors_eager_operands_and_assignment_refusal() {
    for (sql, mode, expected) in [
        (
            "UPDATE Items SET Score=77 WHERE Name LIKE 'A*'",
            None,
            QueryError::Evaluation(EvalError::LikeModeRequired),
        ),
        (
            "UPDATE Items SET Score=77 WHERE Null LIKE CStr(10 \\ 0)",
            Some(LikeMode::Ansi89),
            QueryError::Evaluation(EvalError::DivisionByZero),
        ),
        (
            "UPDATE Items SET Score=77 WHERE False AND Name LIKE CStr(10 \\ 0)",
            Some(LikeMode::Ansi89),
            QueryError::Evaluation(EvalError::DivisionByZero),
        ),
        (
            "UPDATE Items SET Score=IIf(Name LIKE 'A*',1,0)",
            Some(LikeMode::Ansi89),
            QueryError::Unsupported,
        ),
    ] {
        let (editor, calls) = editor();
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let result = plan_batch_update_with_options(
            editor,
            parsed.query(),
            &mut budget(),
            &mut budget(),
            MutationOptions {
                like_mode: mode,
                collation: Some(&DEFAULT_COMPARER),
                ..Default::default()
            },
            &|| false,
        );
        assert!(
            matches!(result, Err(UpdateError::Query(actual)) if actual == expected),
            "{sql}"
        );
        assert_eq!(calls.get(), 0, "no backend prefix for {sql}");
    }
}
