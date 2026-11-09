use super::*;
use crate::comparison::LikeProfile;
use crate::comparison::StringInput;
use proptest::prelude::*;
use std::cell::Cell;
use tablecraft_core::{limits::ResourceBudget, values::Value};

struct Weighted;
fn weights(units: &[u16]) -> Vec<u16> {
    let mut result = Vec::new();
    for &unit in units {
        match unit {
            0 => {}
            0xdf => result.extend([115, 115]),
            0xe9 => result.extend([101, 0x301]),
            65..=90 => result.push(unit + 32),
            _ => result.push(unit),
        }
    }
    result
}
impl Collation for Weighted {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }
    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }
    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        Ok(weights(left).cmp(&weights(right)))
    }
    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        let key = weights(units);
        Ok(key.len() <= 1 || key == [115, 115] || key == [101, 0x301])
    }
}
fn run(text: &str, pattern: &str, mode: LikeMode, col: &dyn Collation) -> Result<Truth, EvalError> {
    super::super::like(
        &Value::Text(text.into()),
        &Value::Text(pattern.into()),
        Some(mode),
        Some(col),
        &mut ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
        None,
    )
}

#[test]
fn unit_counts_literal_runs_classes_and_pattern_spaces_have_explicit_semantics() {
    for (text, pattern, expected) in [
        ("😀", "?", false),
        ("😀", "??", true),
        ("😀", "*", true),
        ("ss", "ß", true),
        ("ß", "ss", true),
        ("ß", "s[]s", true),
        ("ss", "[ß]", true),
        ("tablecraft", "[a-z]", false),
        ("ss", "[!ß]*", false),
        ("ss", "[!b]", true),
        ("ss", "[!ß]s", false),
        ("é", "é", true),
        ("é", "[é]", true),
        ("a", "a ", false),
        ("a ", "a ", true),
        ("a ", "a*", true),
        ("", "[]", true),
        ("a", "[]a", true),
        ("ab", "a[]b", true),
        ("[", "[[]", true),
        ("]", "]", true),
        ("-", "[a-]", true),
        ("2", "#", true),
        ("２", "#", false),
        ("5", "[!0-9]", false),
        ("", "\0", true),
        ("\0", "", false),
    ] {
        assert_eq!(
            run(text, pattern, LikeMode::Ansi89, &Weighted).unwrap(),
            if expected { Truth::True } else { Truth::False },
            "{text:?} LIKE {pattern:?}"
        );
    }
    for (text, pattern, expected) in [
        ("😀", "_", false),
        ("😀", "__", true),
        ("ABC", "a%", true),
        ("a", "[^b]", true),
        ("#", "#", true),
    ] {
        assert_eq!(
            run(text, pattern, LikeMode::Ansi92, &Weighted).unwrap(),
            if expected { Truth::True } else { Truth::False }
        );
    }
}

#[test]
fn reached_patterns_validate_and_null_guards_preserve_existing_order() {
    for pattern in ["[z-a]", "[a-b-c]", "[!]"] {
        for text in ["", "irrelevant"] {
            let expected = if text.is_empty() {
                Ok(Truth::False)
            } else {
                Err(EvalError::InvalidPattern)
            };
            assert_eq!(run(text, pattern, LikeMode::Ansi89, &Weighted), expected);
        }
    }
    for mode in [Some(LikeMode::Ansi89), None] {
        let result = super::super::like(
            &Value::Null,
            &Value::Text("[".into()),
            mode,
            Some(&Weighted),
            &mut ResourceBudget::new(1000, 0, 1000),
            None,
        );
        assert_eq!(
            result,
            if mode.is_some() {
                Ok(Truth::Unknown)
            } else {
                Err(EvalError::LikeModeRequired)
            }
        );
    }
}

#[test]
fn byte_work_and_all_observed_cancellation_checkpoints_propagate() {
    let text = Value::Text("ssb".into());
    let pattern = Value::Text("[ß]*b".into());
    for mut budget in [
        ResourceBudget::new(0, 0, 1000),
        ResourceBudget::new(100000, 0, 1),
    ] {
        assert!(matches!(
            super::super::like(
                &text,
                &pattern,
                Some(LikeMode::Ansi89),
                Some(&Weighted),
                &mut budget,
                None
            ),
            Err(EvalError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    let callback = || {
        calls.set(calls.get() + 1);
        false
    };
    super::super::like(
        &text,
        &pattern,
        Some(LikeMode::Ansi89),
        Some(&Weighted),
        &mut ResourceBudget::new(100000, 0, 100000),
        Some(&callback),
    )
    .unwrap();
    let total = calls.get();
    for stop in 1..=total {
        calls.set(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        assert_eq!(
            super::super::like(
                &text,
                &pattern,
                Some(LikeMode::Ansi89),
                Some(&Weighted),
                &mut ResourceBudget::new(100000, 0, 100000),
                Some(&cancel)
            ),
            Err(EvalError::Cancelled),
            "checkpoint {stop}"
        );
    }
}

struct Identity(bool);
impl Collation for Identity {
    fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(a.cmp(b))
    }
    fn like_profile(&self) -> LikeProfile {
        if self.0 {
            LikeProfile::Utf16Weights
        } else {
            LikeProfile::UnicodeScalar
        }
    }
    fn compare_like_units(&self, a: &[u16], b: &[u16]) -> Result<Ordering, EvalError> {
        Ok(a.cmp(b))
    }
    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        Ok(units.len() == 1)
    }
}
proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn ascii_identity_semantics_match_the_existing_independent_scalar_path(text in "[abc 12!?%_#-]{0,5}",pattern in "[abc 12!?%_#\\[\\]^-]{0,8}") {
        for mode in [LikeMode::Ansi89,LikeMode::Ansi92] {
            prop_assert_eq!(run(&text,&pattern,mode,&Identity(true)),run(&text,&pattern,mode,&Identity(false)));
        }
    }
}

#[test]
fn formal_timing_matrix_preserves_weighted_transition_boundaries() {
    for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
        for (text, pattern, expected) in super::super::timing_tests::cases(mode) {
            assert_eq!(
                run(text, &pattern, mode, &Weighted),
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
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        if left == [122] && right == [97] {
            self.calls.set(self.calls.get() + 1);
            return Err(EvalError::Cancelled);
        }
        Ok(weights(left).cmp(&weights(right)))
    }

    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        Weighted.is_like_unit(units)
    }
}

#[test]
fn weighted_range_providers_wait_for_the_first_nonempty_reachable_class() {
    let provider = RangeFailure {
        calls: Cell::new(0),
    };
    for (text, pattern, expected, calls) in [
        ("", "*[z-a]", Ok(Truth::False), 0),
        ("x", "*a[z-a]", Ok(Truth::False), 0),
        ("ß", "ss[z-a]", Ok(Truth::False), 0),
        ("ßx", "ss[z-a]", Err(EvalError::Cancelled), 1),
    ] {
        assert_eq!(run(text, pattern, LikeMode::Ansi89, &provider), expected);
        assert_eq!(provider.calls.get(), calls);
    }
}

#[test]
fn find_spans_preserve_weighted_consumption_reverse_bounds_and_repeated_occurrences() {
    use crate::{
        comparison::StringSearch,
        predicates::{LikeCase, LikeMatch, find_like_utf16},
    };

    let find = |pattern: &str, matching, search| {
        find_like_utf16(
            StringInput::Text("😀ss ss"),
            StringInput::Text(pattern),
            LikeMode::Ansi89,
            matching,
            search,
            LikeCase::Database,
            &Weighted,
            &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
            None,
        )
        .unwrap()
    };
    assert_eq!(
        find("ß", LikeMatch::Any, StringSearch::ForwardFrom(0)),
        Some(2..4)
    );
    assert_eq!(
        find("ß", LikeMatch::Any, StringSearch::ForwardFrom(4)),
        Some(5..7)
    );
    assert_eq!(
        find("ß", LikeMatch::Any, StringSearch::ReverseThrough(7)),
        Some(5..7)
    );
    assert_eq!(
        find("ß", LikeMatch::Any, StringSearch::ReverseThrough(4)),
        Some(2..4)
    );
    assert_eq!(
        find("??ß", LikeMatch::Start, StringSearch::ForwardFrom(0)),
        Some(0..4)
    );
    assert_eq!(
        find("*ß", LikeMatch::Whole, StringSearch::ForwardFrom(0)),
        Some(0..7)
    );
    assert_eq!(
        find("x", LikeMatch::Any, StringSearch::ForwardFrom(0)),
        None
    );
}

#[test]
fn find_zero_width_and_cursor_bounds_never_split_surrogate_pairs() {
    use crate::{
        comparison::StringSearch,
        predicates::{
            LikeCase, LikeMatch, find_like_utf16, next_like_offset, previous_like_offset,
        },
    };

    let find = |search| {
        find_like_utf16(
            StringInput::Text("😀"),
            StringInput::Text(""),
            LikeMode::Ansi89,
            LikeMatch::Any,
            search,
            LikeCase::Database,
            &Weighted,
            &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
            None,
        )
    };
    assert_eq!(find(StringSearch::ForwardFrom(0)).unwrap(), Some(0..0));
    let units: Vec<_> = "😀".encode_utf16().collect();
    let shortest_nonempty = crate::predicates::PreparedLikeSearch::new(
        StringInput::Text("*"),
        LikeMode::Ansi89,
        LikeMatch::Any,
        LikeCase::Database,
        &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
        None,
    )
    .unwrap();
    assert_eq!(
        shortest_nonempty
            .find(
                StringInput::Text("😀"),
                StringSearch::ForwardFrom(0),
                &Weighted,
                &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
                None,
            )
            .unwrap(),
        Some(0..2)
    );
    assert_eq!(next_like_offset(&units, 0), Some(2));
    assert_eq!(next_like_offset(&units, 2), None);
    assert_eq!(previous_like_offset(&units, 2), Some(0));
    assert_eq!(previous_like_offset(&units, 0), None);
    assert_eq!(find(StringSearch::ForwardFrom(2)).unwrap(), Some(2..2));
    assert_eq!(find(StringSearch::ReverseThrough(2)).unwrap(), Some(2..2));
    assert!(matches!(
        find(StringSearch::ForwardFrom(1)),
        Err(EvalError::InvalidTextMatch)
    ));
}

#[test]
fn find_wildcard_grammars_provider_refusal_and_cumulative_limits_remain_explicit() {
    use crate::{
        comparison::StringSearch,
        predicates::{LikeCase, LikeMatch, find_like_utf16},
    };

    let find = |pattern: &str, mode, case, budget: &mut ResourceBudget, cancelled| {
        find_like_utf16(
            StringInput::Text("x s2 s3"),
            StringInput::Text(pattern),
            mode,
            LikeMatch::Any,
            StringSearch::ForwardFrom(0),
            case,
            &Weighted,
            budget,
            cancelled,
        )
    };
    assert_eq!(
        find(
            "[s]#",
            LikeMode::Ansi89,
            LikeCase::Database,
            &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
            None
        )
        .unwrap(),
        Some(2..4)
    );
    assert_eq!(
        find(
            "s_",
            LikeMode::Ansi92,
            LikeCase::Database,
            &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
            None
        )
        .unwrap(),
        Some(2..4)
    );
    assert!(matches!(
        find(
            "s",
            LikeMode::Ansi89,
            LikeCase::Sensitive,
            &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
            None
        ),
        Err(EvalError::UnsupportedExpression)
    ));
    assert!(matches!(
        find(
            "s",
            LikeMode::Ansi89,
            LikeCase::Database,
            &mut ResourceBudget::new(1, 0, 1),
            None
        ),
        Err(EvalError::Limit(_))
    ));
    assert!(matches!(
        find(
            "s",
            LikeMode::Ansi89,
            LikeCase::Database,
            &mut ResourceBudget::new(1_000_000, 0, 1_000_000),
            Some(&|| true)
        ),
        Err(EvalError::Cancelled)
    ));
}

#[test]
fn prepared_find_reuses_one_pattern_and_merges_origins_without_losing_best_span() {
    use crate::{
        comparison::StringSearch,
        predicates::{LikeCase, LikeMatch, PreparedLikeSearch},
    };

    let mut work = ResourceBudget::new(1_000_000, 0, 1_000_000);
    let prepared = PreparedLikeSearch::new(
        StringInput::Text("a*b"),
        LikeMode::Ansi89,
        LikeMatch::Any,
        LikeCase::Database,
        &mut work,
        None,
    )
    .unwrap();
    for (text, search, expected) in [
        ("xaabxab", StringSearch::ForwardFrom(0), Some(1..4)),
        ("xaabxab", StringSearch::ReverseThrough(7), Some(5..7)),
        ("xaabxab", StringSearch::ForwardFrom(4), Some(5..7)),
        ("zzz", StringSearch::ForwardFrom(0), None),
    ] {
        assert_eq!(
            prepared
                .find(StringInput::Text(text), search, &Weighted, &mut work, None)
                .unwrap(),
            expected
        );
    }
}
