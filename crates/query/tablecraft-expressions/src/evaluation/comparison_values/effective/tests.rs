use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use crate::functions::conversions::text::NumberFormat;
use proptest::prelude::*;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, Single},
};

#[test]
fn numeric_text_rounds_and_checks_the_selected_integral_type() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    for (number, text) in [
        (Value::Byte(2), "2.5"),
        (Value::Int16(2), "1.5"),
        (Value::Int32(4), "3.5"),
        (Value::Int64(9007199254740993), "9007199254740993"),
        (Value::Boolean(true), "-1"),
    ] {
        let text = Value::Text(text.into());
        for (left, right) in [(&number, &text), (&text, &number)] {
            assert_eq!(
                evaluator.compare_effective_numeric_values(Comparison::Equal, left, right),
                Ok(Truth::True)
            );
        }
    }
    for (number, text) in [
        (Value::Byte(1), "256"),
        (Value::Int16(1), "32768"),
        (Value::Int32(1), "2147483648"),
        (Value::Int64(1), "9223372036854775808"),
    ] {
        assert_eq!(
            evaluator.compare_effective_numeric_values(
                Comparison::Equal,
                &number,
                &Value::Text(text.into())
            ),
            Err(EvalError::Overflow)
        );
    }
}

#[test]
fn numeric_pairs_use_reviewed_floating_and_fixed_point_effective_types() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for (left, right) in [
        (
            Value::Single(Single::from_float(1.0)),
            Value::Double(Double::from_float(1.0 + f64::EPSILON)),
        ),
        (
            Value::Currency(Currency::from_scaled(1)),
            Value::Double(Double::from_float(0.000149)),
        ),
        (
            Value::Decimal(Decimal::new(1, 1).unwrap()),
            Value::Double(Double::from_float(0.1)),
        ),
        (
            Value::Int64(9007199254740993),
            Value::Double(Double::from_float(9007199254740992.0)),
        ),
    ] {
        for (a, b) in [(&left, &right), (&right, &left)] {
            assert_eq!(
                evaluator.compare_values(Comparison::Equal, a, b),
                Ok(Truth::True)
            );
            assert_eq!(
                evaluator.compare_effective_numeric_values(Comparison::Equal, a, b),
                Ok(Truth::True)
            );
        }
    }
    assert_eq!(
        evaluator.compare_effective_numeric_values(
            Comparison::Equal,
            &Value::Single(Single::from_float(1.0)),
            &Value::Double(Double::from_float(f64::MAX))
        ),
        Err(EvalError::Overflow)
    );
    assert_eq!(
        evaluator.compare_effective_numeric_values(
            Comparison::Equal,
            &Value::Single(Single::from_float(f32::INFINITY)),
            &Value::Byte(1)
        ),
        Err(EvalError::NonFinite)
    );
}

#[test]
fn explicit_policy_null_and_cancellation_boundaries_remain_visible() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let text = Value::Text("bad".into());
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Equal, &Value::Null, &text),
        Ok(Truth::Unknown)
    );
    assert_eq!(
        evaluator.compare_effective_numeric_values(Comparison::Equal, &Value::Byte(1), &text),
        Err(EvalError::NumberFormatRequired)
    );
    let mut cancelled_budget = ResourceBudget::new(0, 0, 1);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut cancelled_budget, EvalLimits::default())
            .with_cancellation(&|| true)
            .compare_effective_numeric_values(Comparison::Equal, &Value::Byte(1), &text),
        Err(EvalError::Cancelled)
    );
}

#[test]
fn ordinary_numeric_predicates_check_currency_admission_in_both_orders() {
    let mut budget = ResourceBudget::new(65536, 0, 65536);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    let currency = Value::Currency(Currency::from_scaled(0));

    for integer in [
        i64::MIN,
        -922_337_203_685_478,
        922_337_203_685_478,
        i64::MAX,
    ] {
        let integer = Value::Int64(integer);
        for (left, right) in [(&currency, &integer), (&integer, &currency)] {
            assert_eq!(
                evaluator.compare_values(Comparison::Less, left, right),
                Err(EvalError::Overflow)
            );
        }
    }

    assert_eq!(
        evaluator.compare_values(Comparison::Equal, &Value::Null, &Value::Int64(i64::MAX)),
        Ok(Truth::Unknown)
    );
    assert_eq!(
        comparison::compare(Comparison::Less, &currency, &Value::Int64(i64::MAX), None),
        Ok(Truth::True)
    );
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn ordinary_currency_integer_comparison_matches_checked_scaled_model(
        scaled in any::<i64>(),
        integer in any::<i64>(),
    ) {
        let mut budget = ResourceBudget::new(65536, 0, 65536);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        let currency = Value::Currency(Currency::from_scaled(scaled));
        let number = Value::Int64(integer);
        let expected = match integer.checked_mul(10_000) {
            Some(converted) => {
                Ok(if scaled < converted {
                    Truth::True
                } else {
                    Truth::False
                })
            }
            None => Err(EvalError::Overflow),
        };

        prop_assert_eq!(
            evaluator.compare_values(Comparison::Less, &currency, &number),
            expected
        );
        prop_assert_eq!(
            evaluator.compare_values(Comparison::Greater, &number, &currency),
            expected
        );
    }
}
