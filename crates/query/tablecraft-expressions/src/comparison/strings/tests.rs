use super::*;
use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering};

struct Units;

impl Collation for Units {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::CollationFailed)
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn bytes(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(100_000, 0, 100_000)
}

struct Rejects(bool);

impl Collation for Rejects {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::NonFinite)
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        if self.0 {
            StringComparisonProfile::Utf16
        } else {
            StringComparisonProfile::Unicode
        }
    }

    fn compare_utf16(&self, _: &[u16], _: &[u16]) -> Result<Ordering, EvalError> {
        Err(EvalError::CollationFailed)
    }
}

#[test]
fn raw_comparison_requires_opt_in_and_never_retries_provider_failures() {
    for (provider, expected) in [
        (Rejects(false), EvalError::UnsupportedExpression),
        (Rejects(true), EvalError::CollationFailed),
    ] {
        assert_eq!(
            compare_utf16_strings(
                Comparison::Equal,
                StringInput::Utf16Le(&[97, 0]),
                StringInput::Text("a"),
                &provider,
                &mut budget(),
                None,
            ),
            Err(expected)
        );
    }
}

#[cfg(feature = "sql")]
#[test]
fn evaluator_keeps_the_raw_provider_failure_for_publishable_strings() {
    use crate::evaluation::{Evaluator, NoBindings};
    use tablecraft_core::values::{ExpressionString, ExpressionValue, Value};

    let left = ExpressionValue::StringBytes(ExpressionString::from_owned(vec![97, 0]));
    let right = ExpressionValue::Value(Value::Text("a".into()));
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget(), Default::default())
            .with_collation(&Rejects(true))
            .compare_expression_values(Comparison::Equal, &left, &right),
        Err(EvalError::CollationFailed)
    );
}

#[test]
fn complete_raw_units_and_mixed_text_reach_the_explicit_provider() {
    for (left, right, expected) in [
        (&[0xd800][..], &[0xdc00][..], Truth::True),
        (&[0xd800][..], &[0xd800][..], Truth::False),
        (&[0xdc00][..], &[0xd800][..], Truth::False),
    ] {
        assert_eq!(
            compare_utf16_strings(
                Comparison::Less,
                StringInput::Utf16Le(&bytes(left)),
                StringInput::Utf16Le(&bytes(right)),
                &Units,
                &mut budget(),
                None,
            ),
            Ok(expected)
        );
    }

    let unaligned = [255, 0, 216, 255];
    assert_eq!(
        compare_utf16_strings(
            Comparison::Greater,
            StringInput::Utf16Le(&unaligned[1..3]),
            StringInput::Text("a"),
            &Units,
            &mut budget(),
            None,
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        compare_utf16_strings(
            Comparison::Equal,
            StringInput::Text("😀"),
            StringInput::Utf16Le(&[61, 216, 0, 222]),
            &Units,
            &mut budget(),
            None,
        ),
        Ok(Truth::True)
    );
}

#[test]
fn raw_comparison_refuses_odd_units_and_checks_budgets_and_cancellation() {
    assert_eq!(
        compare_utf16_strings(
            Comparison::Equal,
            StringInput::Utf16Le(&[65]),
            StringInput::Text("a"),
            &Units,
            &mut budget(),
            None,
        ),
        Err(EvalError::UnrepresentableText)
    );

    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    let left = StringInput::Utf16Le(&[0, 216, 65, 0]);
    let right = StringInput::Text("a");
    compare_utf16_strings(
        Comparison::Less,
        left,
        right,
        &Units,
        &mut budget(),
        Some(&observe),
    )
    .unwrap();
    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancel = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        assert_eq!(
            compare_utf16_strings(
                Comparison::Less,
                left,
                right,
                &Units,
                &mut budget(),
                Some(&cancel),
            ),
            Err(EvalError::Cancelled)
        );
    }

    for mut budget in [
        ResourceBudget::new(0, 0, 1000),
        ResourceBudget::new(1000, 0, 1),
    ] {
        assert!(matches!(
            compare_utf16_strings(Comparison::Equal, left, right, &Units, &mut budget, None),
            Err(EvalError::Limit(_))
        ));
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]

    #[test]
    fn raw_comparison_matches_independent_unsigned_unit_order(
        left in prop::collection::vec(any::<u16>(), 0..9),
        right in prop::collection::vec(any::<u16>(), 0..9),
    ) {
        let order = left.cmp(&right);
        for (operator, expected) in [
            (Comparison::Equal, order == Ordering::Equal),
            (Comparison::NotEqual, order != Ordering::Equal),
            (Comparison::Less, order == Ordering::Less),
            (Comparison::LessOrEqual, order != Ordering::Greater),
            (Comparison::Greater, order == Ordering::Greater),
            (Comparison::GreaterOrEqual, order != Ordering::Less),
        ] {
            let result = compare_utf16_strings(
                operator,
                StringInput::Utf16Le(&bytes(&left)),
                StringInput::Utf16Le(&bytes(&right)),
                &Units,
                &mut budget(),
                None,
            );
            prop_assert_eq!(result, Ok(if expected { Truth::True } else { Truth::False }));
        }
    }
}
