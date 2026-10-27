use super::*;
use crate::{
    comparison::Collation,
    evaluation::NoBindings,
    functions::conversions::text::NumberFormat,
    predicates::{LikeMode, like},
};
use std::cmp::Ordering;
use tablecraft_core::{limits::ResourceBudget, values::ExpressionString};

struct ComparisonOnly;

impl Collation for ComparisonOnly {
    fn compare(&self, _left: &str, _right: &str) -> Result<Ordering, EvalError> {
        panic!("Ordering cannot implement wildcard matching");
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Unsupported
    }
}

#[test]
fn unsupported_profile_rejects_wildcard_only_patterns_in_both_entrypoints() {
    for (mode, patterns) in [
        (LikeMode::Ansi89, ["*", "?", "abc"]),
        (LikeMode::Ansi92, ["%", "_", "abc"]),
    ] {
        for pattern in patterns {
            let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
            let value = Value::Text("abc".into());
            let pattern = Value::Text(pattern.into());
            assert_eq!(
                like(
                    &value,
                    &pattern,
                    Some(mode),
                    Some(&ComparisonOnly),
                    &mut budget,
                    None
                ),
                Err(EvalError::UnsupportedExpression)
            );
            let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_like_mode(mode)
                .with_collation(&ComparisonOnly);
            assert_eq!(
                evaluator.apply_like(&value, &pattern, false),
                Err(EvalError::UnsupportedExpression)
            );
            let raw = ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 0xd8]));
            assert_eq!(
                evaluator.apply_expression_like(&raw, &ExpressionValue::Value(pattern), false),
                Err(EvalError::UnsupportedExpression)
            );
        }
    }
}

#[test]
fn unsupported_matching_preserves_mode_null_and_conversion_phases() {
    let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_collation(&ComparisonOnly);
    assert_eq!(
        evaluator.apply_like(&Value::Null, &Value::Null, false),
        Err(EvalError::LikeModeRequired)
    );
    let mut evaluator = evaluator.with_like_mode(LikeMode::Ansi89);
    assert_eq!(
        evaluator.apply_like(&Value::Null, &Value::Binary(vec![1]), false),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluator.apply_like(&Value::Int32(1), &Value::Text("*".into()), false),
        Err(EvalError::NumberFormatRequired)
    );
    let mut evaluator = evaluator.with_number_format(NumberFormat::Invariant);
    assert_eq!(
        evaluator.apply_like(&Value::Int32(1), &Value::Text("*".into()), false),
        Err(EvalError::UnsupportedExpression)
    );
}
