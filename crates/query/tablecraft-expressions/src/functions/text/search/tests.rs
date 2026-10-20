use super::*;
use crate::functions::text::{apply, apply_with_collation};
use proptest::prelude::*;
use std::{cell::Cell, ops::Range};
fn text(s: &str) -> Value {
    Value::Text(s.into())
}
fn forward(haystack: &str, needle: &str, start: i32) -> Result<TextResult<'static>, EvalError> {
    own(apply(
        TextFunction::InStr,
        &[
            Value::Int32(start),
            text(haystack),
            text(needle),
            Value::Int16(0),
        ],
        || Ok(()),
    )?)
}
fn reverse(haystack: &str, needle: &str, start: i32) -> Result<TextResult<'static>, EvalError> {
    own(apply(
        TextFunction::InStrRev,
        &[text(haystack), text(needle), Value::Int32(start)],
        || Ok(()),
    )?)
}
fn own(result: TextResult<'_>) -> Result<TextResult<'static>, EvalError> {
    Ok(match result {
        TextResult::Position(value) => TextResult::Position(value),
        TextResult::Null => TextResult::Null,
        _ => unreachable!(),
    })
}
#[test]
fn utf16_positions_overlap_reverse_end_bounds_and_empty_strings_are_explicit() {
    for (haystack, needle, start, expected) in [
        ("XXpXXpXXPXXP", "P", 1, 9),
        ("ababa", "aba", 2, 3),
        ("😀x😀", "x", 1, 3),
        ("😀x😀", "😀", 2, 4),
        ("a\0b\0", "\0", 3, 4),
        ("é", "e\u{301}", 1, 0),
        ("abc", "", 2, 2),
        ("", "", 1, 0),
        ("abc", "a", 4, 0),
        ("abc", "", 4, 0),
    ] {
        assert_eq!(
            forward(haystack, needle, start),
            Ok(TextResult::Position(expected))
        );
    }
    for (haystack, needle, start, expected) in [
        ("ababa", "aba", -1, 3),
        ("ababa", "aba", 4, 1),
        ("ababa", "aba", 2, 0),
        ("😀x😀", "😀", 4, 1),
        ("😀x😀", "😀", -1, 4),
        ("abc", "", -1, 3),
        ("", "", -1, 0),
        ("abc", "a", 4, 0),
    ] {
        assert_eq!(
            reverse(haystack, needle, start),
            Ok(TextResult::Position(expected))
        );
    }
}
#[test]
fn modes_positions_and_eager_null_validation_do_not_coerce_text() {
    assert_eq!(forward("x", "x", 0), Err(EvalError::Domain));
    assert_eq!(reverse("x", "x", -2), Err(EvalError::Domain));
    assert_eq!(reverse("x", "x", 0), Err(EvalError::Domain));
    for (function, args, expected) in [
        (
            TextFunction::InStr,
            vec![text("a"), text("a")],
            Err(EvalError::CollationRequired),
        ),
        (
            TextFunction::InStrRev,
            vec![text("a"), text("a")],
            Ok(TextResult::Position(1)),
        ),
        (
            TextFunction::InStrRev,
            vec![text("a"), text("a"), Value::Int16(-1), Value::Int16(-1)],
            Err(EvalError::CollationRequired),
        ),
        (
            TextFunction::InStr,
            vec![Value::Null, Value::Null, text("a")],
            Err(EvalError::InvalidNullConversion),
        ),
        (
            TextFunction::InStrRev,
            vec![Value::Null, text("a"), Value::Int16(-1), Value::Null],
            Err(EvalError::InvalidNullConversion),
        ),
        (
            TextFunction::InStrRev,
            vec![Value::Null, text("a")],
            Ok(TextResult::Null),
        ),
        (
            TextFunction::InStrRev,
            vec![Value::Int16(1), text("1")],
            Err(EvalError::TypeMismatch),
        ),
        (
            TextFunction::InStr,
            vec![
                Value::Double(tablecraft_core::values::Double::from_float(1.5)),
                text("aab"),
                text("a"),
                Value::Double(tablecraft_core::values::Double::from_float(0.5)),
            ],
            Ok(TextResult::Position(2)),
        ),
    ] {
        assert_eq!(apply(function, &args, || Ok(())), expected);
    }
}
struct Host {
    result: Range<usize>,
    calls: Cell<usize>,
}
impl Collation for Host {
    fn compare(&self, _: &str, _: &str) -> Result<std::cmp::Ordering, EvalError> {
        unreachable!()
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
        assert_eq!((haystack, needle), ("Straße", "SS"));
        assert_eq!(mode, StringCompareMode::Text);
        assert!(matches!(
            search,
            StringSearch::ForwardFrom(0) | StringSearch::ReverseThrough(6)
        ));
        self.calls.set(self.calls.get() + 1);
        // A separately supplied locale provider may match unequal UTF-16 lengths.
        Ok(Some(self.result.clone()))
    }
}
#[test]
fn locale_search_has_its_own_callback_and_validates_provider_ranges() {
    let host = Host {
        result: 4..5,
        calls: Cell::new(0),
    };
    for (function, args) in [
        (TextFunction::InStr, vec![text("Straße"), text("SS")]),
        (
            TextFunction::InStrRev,
            vec![
                text("Straße"),
                text("SS"),
                Value::Int16(-1),
                Value::Int16(-1),
            ],
        ),
    ] {
        assert_eq!(
            apply_with_collation(function, &args, Some(&host), || Ok(())),
            Ok(TextResult::Position(5))
        );
    }
    assert_eq!(host.calls.get(), 2);
    let bad = Host {
        result: 4..7,
        calls: Cell::new(0),
    };
    assert_eq!(
        apply_with_collation(
            TextFunction::InStr,
            &[text("Straße"), text("SS")],
            Some(&bad),
            || Ok(())
        ),
        Err(EvalError::InvalidTextMatch)
    );
}
#[test]
fn adversarial_scans_and_provider_prescans_obey_work_limits() {
    let mut calls = 0;
    let mut step = || {
        calls += 1;
        if calls > 40 {
            Err(EvalError::Cancelled)
        } else {
            Ok(())
        }
    };
    assert_eq!(
        apply(
            TextFunction::InStr,
            &[
                Value::Int16(1),
                text("aaaaaaaaaa"),
                text("aaaaab"),
                Value::Int16(0)
            ],
            &mut step
        ),
        Err(EvalError::Cancelled)
    );
    let host = Host {
        result: 4..5,
        calls: Cell::new(0),
    };
    assert_eq!(
        apply_with_collation(
            TextFunction::InStr,
            &[text("Straße"), text("SS")],
            Some(&host),
            || Err(EvalError::Cancelled)
        ),
        Err(EvalError::Cancelled)
    );
    assert_eq!(host.calls.get(), 0);
}
proptest! {
    #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(0x49535452), ..ProptestConfig::default() })]
    #[test]
    fn binary_search_matches_independent_utf16_windows(haystack in ".{0,30}", needle in ".{1,6}", start in 1usize..35) {
        let hay: Vec<_> = haystack.encode_utf16().collect();
        let pattern: Vec<_> = needle.encode_utf16().collect();
        let matches: Vec<_> = hay.windows(pattern.len()).enumerate().filter(|(_,v)| *v == pattern).map(|(i,_)| i).collect();
        let expected = matches.iter().find(|&&i| i >= start-1).map_or(0, |i| (i+1) as i32);
        prop_assert_eq!(forward(&haystack,&needle,start as i32),Ok(TextResult::Position(expected)));
        let expected = if start > hay.len() { 0 } else { matches.iter().rev().find(|&&i| i + pattern.len() <= start).map_or(0, |i| (i+1) as i32) };
        prop_assert_eq!(reverse(&haystack,&needle,start as i32),Ok(TextResult::Position(expected)));
    }
}
