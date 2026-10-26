//! A callback cannot publish a reverse position beyond the requested start.
use super::*;
use crate::functions::text::apply_with_collation;
use std::{cell::Cell, cmp::Ordering};

struct Provider {
    result: Result<Option<Range<usize>>, EvalError>,
    calls: Cell<usize>,
}

impl Collation for Provider {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("search must use its explicit callback")
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Text)
    }

    fn find(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        assert_eq!((haystack, needle), ("abcd", "b"));
        assert_eq!(search, StringSearch::ReverseThrough(2));
        assert!(matches!(
            mode,
            StringCompareMode::Text | StringCompareMode::Database
        ));
        self.calls.set(self.calls.get() + 1);
        self.result.clone()
    }
}

fn call(provider: &Provider, mode: i16) -> Result<i32, EvalError> {
    apply_with_collation(
        TextFunction::InStrRev,
        &[
            Value::Text("abcd".into()),
            Value::Text("b".into()),
            Value::Int16(2),
            Value::Int16(mode),
        ],
        Some(provider),
        || Ok(()),
    )
    .map(|result| match result {
        TextResult::Position(position) => position,
        _ => panic!("non-NULL search must produce a position"),
    })
}

#[test]
fn unicode_callbacks_preserve_interior_empty_spans_but_reject_the_reverse_boundary() {
    for mode in [-1, 1, 2] {
        for (range, expected) in [
            (0..0, Ok(1)),
            (1..1, Ok(2)),
            (1..2, Ok(2)),
            (2..2, Err(EvalError::InvalidTextMatch)),
        ] {
            let provider = Provider {
                result: Ok(Some(range)),
                calls: Cell::new(0),
            };
            assert_eq!(call(&provider, mode), expected);
            assert_eq!(provider.calls.get(), 1);
        }
    }
    let provider = Provider {
        result: Err(EvalError::CollationFailed),
        calls: Cell::new(0),
    };
    assert_eq!(call(&provider, 0), Ok(2));
    assert_eq!(provider.calls.get(), 0);
}

#[test]
fn callback_range_validation_agrees_with_a_finite_position_bound_model() {
    for length in 1..=8 {
        for bound in 1..=length {
            for start in 0..=length + 1 {
                for end in 0..=length + 1 {
                    let range = Some(Range { start, end });
                    // Accepted positions are one-based and must be at most the
                    // supplied reverse start. Empty spans elsewhere stay legal.
                    let position = start + 1;
                    let reverse = start <= end && position <= bound && end <= bound;
                    assert_eq!(
                        validate_match(&range, length, StringSearch::ReverseThrough(bound)).is_ok(),
                        reverse,
                        "length {length}, bound {bound}, span {start}..{end}"
                    );
                    let forward =
                        start <= end && start >= bound - 1 && start < length && end <= length;
                    assert_eq!(
                        validate_match(&range, length, StringSearch::ForwardFrom(bound - 1))
                            .is_ok(),
                        forward,
                        "length {length}, bound {bound}, span {start}..{end}"
                    );
                }
            }
        }
    }
}

#[test]
fn unicode_callback_failures_remain_typed_in_every_callback_mode() {
    for mode in [-1, 1, 2] {
        for error in [EvalError::Cancelled, EvalError::CollationFailed] {
            let provider = Provider {
                result: Err(error),
                calls: Cell::new(0),
            };
            assert_eq!(call(&provider, mode), Err(error));
            assert_eq!(provider.calls.get(), 1);
        }
    }
}
