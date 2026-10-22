use super::*;
use crate::evaluation::NoBindings;
use proptest::prelude::*;
use tablecraft_core::limits::{Resource, ResourceBudget};

#[test]
fn opaque_and_mixed_error_comparisons_never_guess_a_numeric_conversion() {
    let mut budget = ResourceBudget::new(4096, 0, 4096);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    for (left, right) in [(0, 65536), (u32::MAX, 0), (u32::MAX, u32::MAX)] {
        assert_eq!(
            evaluator.compare_expression_values(
                Comparison::Equal,
                &ExpressionValue::Error(left),
                &ExpressionValue::Error(right),
            ),
            Err(EvalError::UnsupportedExpression)
        );
    }
    for scalar in [Value::Null, Value::Int32(0), Value::Text("Error 0".into())] {
        for (left, right) in [
            (
                ExpressionValue::Error(0),
                ExpressionValue::Value(scalar.clone()),
            ),
            (ExpressionValue::Value(scalar), ExpressionValue::Error(0)),
        ] {
            assert_eq!(
                evaluator.compare_expression_values(Comparison::Equal, &left, &right),
                Err(EvalError::TypeMismatch)
            );
        }
    }
    assert_eq!(
        evaluator.compare_expression_values(
            Comparison::Equal,
            &ExpressionValue::Empty,
            &ExpressionValue::Value(Value::Int32(0)),
        ),
        Err(EvalError::NonScalarExpression)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn standard_error_pairs_compare_like_the_independent_unsigned_code_model(
        left in any::<u16>(),
        right in any::<u16>(),
    ) {
        let mut budget = ResourceBudget::new(4096, 0, 4096);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
        for (operator, expected) in [
            (Comparison::Equal, left == right),
            (Comparison::NotEqual, left != right),
            (Comparison::Less, left < right),
            (Comparison::LessOrEqual, left <= right),
            (Comparison::Greater, left > right),
            (Comparison::GreaterOrEqual, left >= right),
        ] {
            let actual = evaluator.compare_expression_values(
                operator,
                &ExpressionValue::Error(u32::from(left)),
                &ExpressionValue::Error(u32::from(right)),
            );
            prop_assert_eq!(actual, Ok(if expected { Truth::True } else { Truth::False }));
        }
    }
}

#[test]
fn error_comparison_uses_shared_work_and_checks_cancellation_before_code_interpretation() {
    let error = ExpressionValue::Error(1);
    let mut budget = ResourceBudget::new(0, 0, 1);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, Default::default());
    assert_eq!(
        evaluator.compare_expression_values(Comparison::Equal, &error, &error),
        Ok(Truth::True)
    );
    assert!(matches!(
        evaluator.compare_expression_values(Comparison::Equal, &error, &error),
        Err(EvalError::Limit(limit)) if limit.resource == Resource::Steps
    ));

    let mut budget = ResourceBudget::new(0, 0, 0);
    let mut evaluator =
        Evaluator::new(&NoBindings, &mut budget, Default::default()).with_cancellation(&|| true);
    assert_eq!(
        evaluator.compare_expression_values(
            Comparison::Equal,
            &ExpressionValue::Error(u32::MAX),
            &error,
        ),
        Err(EvalError::Cancelled)
    );
}
