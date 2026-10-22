use crate::{comparison::Comparison, logic::Truth};

/// Inclusive Access-style BETWEEN, allowing either written bound order.
/// All four comparisons run in order unless one fails. The supplied comparator
/// owns collation, cancellation/work accounting and its typed error policy.
pub fn between<E, T: ?Sized>(
    value: &T,
    first: &T,
    second: &T,
    negated: bool,
    mut compare: impl FnMut(Comparison, &T, &T) -> Result<Truth, E>,
) -> Result<Truth, E> {
    let forward = compare(Comparison::GreaterOrEqual, value, first)?.and(compare(
        Comparison::LessOrEqual,
        value,
        second,
    )?);
    let reverse = compare(Comparison::GreaterOrEqual, value, second)?.and(compare(
        Comparison::LessOrEqual,
        value,
        first,
    )?);
    let result = forward.or(reverse);
    Ok(if negated { result.negate() } else { result })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EvalError, comparison::compare};
    use proptest::prelude::*;
    use tablecraft_core::values::Value;
    #[test]
    fn reversed_bounds_null_and_negation_use_shared_numeric_truth() {
        let compare = |op, a: &Value, b: &Value| compare(op, a, b, None);
        assert_eq!(
            between(
                &Value::Int64(2),
                &Value::Int64(3),
                &Value::Int64(1),
                false,
                compare
            ),
            Ok(Truth::True)
        );
        assert_eq!(
            between(
                &Value::Int64(2),
                &Value::Int64(3),
                &Value::Int64(1),
                true,
                compare
            ),
            Ok(Truth::False)
        );
        assert_eq!(
            between(
                &Value::Null,
                &Value::Int64(3),
                &Value::Int64(1),
                false,
                compare
            ),
            Ok(Truth::Unknown)
        );
    }
    #[test]
    fn range_does_not_skip_reverse_comparison_failure_after_a_true_forward_match() {
        let mut calls = 0;
        let result = between(
            &Value::Int64(2),
            &Value::Int64(1),
            &Value::Int64(3),
            false,
            |_, _, _| {
                calls += 1;
                if calls == 3 {
                    Err(EvalError::Cancelled)
                } else {
                    Ok(Truth::True)
                }
            },
        );
        assert_eq!(result, Err(EvalError::Cancelled));
        assert_eq!(calls, 3);
    }
    proptest! {
        #![proptest_config(ProptestConfig {cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4245545745454e),..Default::default()})]
        #[test]
        fn exact_integer_ranges_match_an_independent_closed_interval(value in any::<i64>(),first in any::<i64>(),second in any::<i64>(),negated in any::<bool>()) {
            let inside=value>=first.min(second)&&value<=first.max(second);
            let actual=between(&Value::Int64(value),&Value::Int64(first),&Value::Int64(second),negated,|op,a,b|compare(op,a,b,None)).unwrap();
            prop_assert_eq!(actual, if inside!=negated {Truth::True}else{Truth::False});
        }
    }
}
