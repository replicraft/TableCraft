//! Explicit error-frontier goldens from the MS-VBAL element-by-element model.
use super::*;
use std::{cell::Cell, cmp::Ordering};

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

pub(super) fn cases(mode: LikeMode) -> Vec<(&'static str, String, Result<Truth, EvalError>)> {
    use Truth::{False, True};
    let invalid = Err(EvalError::InvalidPattern);
    [
        ("x", "a[", Ok(False)),
        ("ab", "a[", invalid),
        ("a", "a[", Ok(False)),
        ("", "[", Ok(False)),
        ("", "[z-a]", Ok(False)),
        ("", "[!]", Ok(False)),
        ("", "[]", Ok(True)),
        ("", "[][", Ok(False)),
        ("", "[]*", Ok(True)),
        ("", "*[", invalid),
        ("", "*[z-a]", Ok(False)),
        ("", "a*[", Ok(False)),
        ("x", "a*[", Ok(False)),
        ("x", "?[", Ok(False)),
        ("xy", "?[", invalid),
        ("x", "#[", Ok(False)),
        ("1", "#[", Ok(False)),
        ("1x", "#[", invalid),
        ("b", "[ab][", Ok(False)),
        ("bx", "[ab][", invalid),
        ("cx", "[ab][", Ok(False)),
        ("x", "*a[", invalid),
        ("x", "*a[z-a]", Ok(False)),
        ("ab", "*a[z-a]", invalid),
        ("x", "a[b-c-d]", Ok(False)),
        ("ab", "a[b-c-d]", invalid),
        ("x", "*[b-c-d]", invalid),
        ("x", "[!a][", Ok(False)),
        ("ax", "[!a][", Ok(False)),
        ("bx", "[!a][", invalid),
        ("x", "a[][", Ok(False)),
        ("a", "a[][", Ok(False)),
        ("ab", "a[][", invalid),
        ("ab", "a[z-a]*", invalid),
        ("x", "a[z-a]*", Ok(False)),
        ("x", "*a[z-a]*", Ok(False)),
    ]
    .into_iter()
    .map(|(text, pattern, expected)| {
        let pattern = if mode == LikeMode::Ansi92 {
            pattern
                .replace('*', "%")
                .replace('?', "_")
                .replace('!', "^")
                .replace('#', "[0-9]")
        } else {
            pattern.to_owned()
        };
        (text, pattern, expected)
    })
    .collect()
}

fn run(text: &str, pattern: &str, mode: LikeMode) -> Result<Truth, EvalError> {
    like(
        &Value::Text(text.into()),
        &Value::Text(pattern.into()),
        Some(mode),
        Some(&Ordinal),
        &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
        None,
    )
}

#[test]
fn malformed_elements_are_errors_only_at_the_formal_matching_frontier() {
    for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
        for (text, pattern, expected) in cases(mode) {
            assert_eq!(
                run(text, &pattern, mode),
                expected,
                "{text:?} / {pattern:?}"
            );
        }
    }
}

struct RangeFailure {
    calls: Cell<usize>,
}

impl Collation for RangeFailure {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        if left == "z" && right == "a" {
            self.calls.set(self.calls.get() + 1);
            return Err(EvalError::Cancelled);
        }
        Ok(left.cmp(right))
    }
}

#[test]
fn star_grammar_validation_does_not_activate_unreached_range_providers() {
    let provider = RangeFailure {
        calls: Cell::new(0),
    };
    for (text, pattern, expected, calls) in [
        ("x", "a[z-a]", Ok(Truth::False), 0),
        ("x", "*a[z-a]", Ok(Truth::False), 0),
        ("", "*[z-a]", Ok(Truth::False), 0),
        ("ab", "a[z-a]", Err(EvalError::Cancelled), 1),
    ] {
        let result = like(
            &Value::Text(text.into()),
            &Value::Text(pattern.into()),
            Some(LikeMode::Ansi89),
            Some(&provider),
            &mut ResourceBudget::new(65536, 0, 10000),
            None,
        );
        assert_eq!(result, expected, "{text:?} / {pattern:?}");
        assert_eq!(provider.calls.get(), calls);
    }
}

#[test]
fn deferred_syntax_does_not_swallow_operational_limits_or_cancellation() {
    let text = Value::Text("x".into());
    let pattern = Value::Text("a*[".into());
    let checks = Cell::new(0);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        like(
            &text,
            &pattern,
            Some(LikeMode::Ansi89),
            Some(&Ordinal),
            &mut budget,
            Some(&|| {
                checks.set(checks.get() + 1);
                false
            }),
        ),
        Ok(Truth::False)
    );
    for stop in 1..=checks.get() {
        let calls = Cell::new(0);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        let result = like(
            &text,
            &pattern,
            Some(LikeMode::Ansi89),
            Some(&Ordinal),
            &mut budget,
            Some(&|| {
                calls.set(calls.get() + 1);
                calls.get() >= stop
            }),
        );
        assert_eq!(result, Err(EvalError::Cancelled), "checkpoint {stop}");
    }
    for (bytes, steps) in [(0, 10000), (65536, 0)] {
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        assert!(matches!(
            like(
                &text,
                &pattern,
                Some(LikeMode::Ansi89),
                Some(&Ordinal),
                &mut budget,
                None
            ),
            Err(EvalError::Limit(_))
        ));
    }
}
