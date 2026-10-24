use super::*;
use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering};

struct Units;

impl Collation for Units {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }

    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        Ok(units.len() == 1)
    }
}

fn bytes(units: &[u16]) -> Vec<u8> {
    units.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

fn run(text: &[u16], pattern: &[u16], mode: LikeMode) -> Result<Truth, EvalError> {
    like_utf16(
        LikeString::Utf16Le(&bytes(text)),
        LikeString::Utf16Le(&bytes(pattern)),
        mode,
        &Units,
        &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
        None,
    )
}

#[test]
fn raw_like_retains_surrogates_classes_nuls_and_unaligned_unit_ranges() {
    for (text, pattern, expected) in [
        (&[0xd800][..], &[0xd800][..], true),
        (&[0xd800][..], &[0xdc00][..], false),
        (&[0xd800][..], &[63][..], true),
        (&[0xd83d, 0xde00][..], &[63][..], false),
        (&[0xd83d, 0xde00][..], &[63, 63][..], true),
        (&[0xdc00][..], &[91, 0xd800, 45, 0xdfff, 93][..], true),
        (&[0xdc00][..], &[91, 33, 0xd800, 45, 0xdfff, 93][..], false),
        (&[0][..], &[0][..], true),
        (&[0][..], &[][..], false),
    ] {
        assert_eq!(
            run(text, pattern, LikeMode::Ansi89),
            Ok(if expected { Truth::True } else { Truth::False })
        );
    }

    let unaligned = [255, 0, 216, 255];
    assert_eq!(
        like_utf16(
            LikeString::Utf16Le(&unaligned[1..3]),
            LikeString::Text("_"),
            LikeMode::Ansi92,
            &Units,
            &mut ResourceBudget::new(1000, 0, 1000),
            None,
        ),
        Ok(Truth::True)
    );
    assert_eq!(
        run(&[], &[91], LikeMode::Ansi89),
        Err(EvalError::InvalidPattern)
    );
}

#[test]
fn raw_like_checks_odd_ranges_budgets_and_every_observed_checkpoint() {
    for (text, pattern) in [(&[65][..], &[63, 0][..]), (&[65, 0][..], &[63][..])] {
        assert_eq!(
            like_utf16(
                LikeString::Utf16Le(text),
                LikeString::Utf16Le(pattern),
                LikeMode::Ansi89,
                &Units,
                &mut ResourceBudget::new(1000, 0, 1000),
                None,
            ),
            Err(EvalError::UnrepresentableText)
        );
    }

    let text = bytes(&[0xd800, 65, 0xdc00]);
    let pattern = bytes(&[42, 91, 65, 93, 63]);
    let calls = Cell::new(0);
    let observe = || {
        calls.set(calls.get() + 1);
        false
    };
    assert_eq!(
        like_utf16(
            LikeString::Utf16Le(&text),
            LikeString::Utf16Le(&pattern),
            LikeMode::Ansi89,
            &Units,
            &mut ResourceBudget::new(100_000, 0, 100_000),
            Some(&observe),
        ),
        Ok(Truth::True)
    );

    for stop in 1..=calls.get() {
        let seen = Cell::new(0);
        let cancel = || {
            seen.set(seen.get() + 1);
            seen.get() == stop
        };
        assert_eq!(
            like_utf16(
                LikeString::Utf16Le(&text),
                LikeString::Utf16Le(&pattern),
                LikeMode::Ansi89,
                &Units,
                &mut ResourceBudget::new(100_000, 0, 100_000),
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
            like_utf16(
                LikeString::Utf16Le(&text),
                LikeString::Utf16Le(&pattern),
                LikeMode::Ansi89,
                &Units,
                &mut budget,
                None,
            ),
            Err(EvalError::Limit(_))
        ));
    }
}

fn model(text: &[u16], pattern: &[u16], many: u16, one: u16) -> bool {
    let Some((&head, tail)) = pattern.split_first() else {
        return text.is_empty();
    };
    if head == many {
        return (0..=text.len()).any(|start| model(&text[start..], tail, many, one));
    }
    let Some((&unit, rest)) = text.split_first() else {
        return false;
    };
    (head == one || head == unit) && model(rest, tail, many, one)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn raw_like_agrees_with_an_independent_exhaustive_unit_glob(
        text in prop::collection::vec(
            prop_oneof![any::<u16>(), Just(0xd800_u16), Just(0xdc00_u16), Just(65_u16)],
            0..6,
        ),
        tokens in prop::collection::vec(0_u8..5, 0..6),
        literal in prop_oneof![Just(0xd800_u16), Just(0xdc00_u16), Just(65_u16)],
    ) {
        for (mode, many, one) in [
            (LikeMode::Ansi89, 42_u16, 63_u16),
            (LikeMode::Ansi92, 37_u16, 95_u16),
        ] {
            let pattern: Vec<_> = tokens.iter().map(|token| match token {
                0 => many,
                1 => one,
                _ => literal,
            }).collect();
            let expected = if model(&text, &pattern, many, one) {
                Truth::True
            } else {
                Truth::False
            };
            prop_assert_eq!(run(&text, &pattern, mode), Ok(expected));
        }
    }
}
