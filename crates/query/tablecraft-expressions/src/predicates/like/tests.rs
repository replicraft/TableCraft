use super::*;
use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering};
struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(a.cmp(b))
    }
}
fn matches(text: &str, pattern: &str, mode: LikeMode) -> Result<Truth, EvalError> {
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
fn wildcard_goldens_cover_modes_lists_ranges_empty_unicode_and_literals() {
    for (text, pattern, mode, expected) in [
        ("", "", LikeMode::Ansi89, true),
        ("", "*", LikeMode::Ansi89, true),
        ("abc", "**a**?c**", LikeMode::Ansi89, true),
        ("a\nb", "a?b", LikeMode::Ansi89, true),
        ("é🙂", "??", LikeMode::Ansi89, true),
        ("é🙂", "?", LikeMode::Ansi89, false),
        ("2", "#", LikeMode::Ansi89, true),
        ("٢", "#", LikeMode::Ansi89, false),
        ("bell", "b[ae]ll", LikeMode::Ansi89, true),
        ("bill", "b[!ae]ll", LikeMode::Ansi89, true),
        ("ball", "b[!ae]ll", LikeMode::Ansi89, false),
        ("bbd", "b[a-c]d", LikeMode::Ansi89, true),
        ("B", "[a-z]", LikeMode::Ansi89, false),
        ("-", "[-ab]", LikeMode::Ansi89, true),
        ("-", "[ab-]", LikeMode::Ansi89, true),
        ("a", "[!-]", LikeMode::Ansi89, true),
        ("[]", "[[]]", LikeMode::Ansi89, true),
        ("", "[]", LikeMode::Ansi89, true),
        ("ab", "a[]b", LikeMode::Ansi89, true),
        ("*?#", "[*][?][#]", LikeMode::Ansi89, true),
        ("!", "!", LikeMode::Ansi89, true),
        ("%_", "%_", LikeMode::Ansi89, true),
        ("abc", "a%", LikeMode::Ansi89, false),
        ("abc", "a%", LikeMode::Ansi92, true),
        ("é🙂", "__", LikeMode::Ansi92, true),
        ("z", "[^a-c]", LikeMode::Ansi92, true),
        ("b", "[^a-c]", LikeMode::Ansi92, false),
        ("*?#", "*?#", LikeMode::Ansi92, true),
        ("x", "#", LikeMode::Ansi92, false),
        ("abc ", "abc", LikeMode::Ansi89, false),
    ] {
        assert_eq!(
            matches(text, pattern, mode),
            Ok(if expected { Truth::True } else { Truth::False }),
            "{text:?} {pattern:?} {mode:?}"
        );
    }
    for mode in [LikeMode::Ansi89, LikeMode::Ansi92] {
        for pattern in ["[", "x[abc", "[z-a]", "[a-b-c]"] {
            // Compile all syntax even when no possible prefix could match.
            assert_eq!(matches("", pattern, mode), Err(EvalError::InvalidPattern));
        }
    }
}
#[test]
fn policies_null_types_errors_and_cancellation_remain_explicit() {
    let text = Value::Text("a".into());
    let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
    assert_eq!(
        like(&text, &text, None, Some(&Ordinal), &mut budget, None),
        Err(EvalError::LikeModeRequired)
    );
    assert_eq!(
        like(
            &text,
            &text,
            Some(LikeMode::Ansi89),
            None,
            &mut budget,
            None
        ),
        Err(EvalError::CollationRequired)
    );
    assert_eq!(
        like(
            &Value::Null,
            &text,
            Some(LikeMode::Ansi89),
            None,
            &mut budget,
            None
        ),
        Ok(Truth::Unknown)
    );
    assert_eq!(
        like(
            &Value::Int32(1),
            &text,
            Some(LikeMode::Ansi89),
            Some(&Ordinal),
            &mut budget,
            None
        ),
        Err(EvalError::TypeMismatch)
    );
    let cancelled = Cell::new(false);
    struct Cancels<'a>(&'a Cell<bool>);
    impl Collation for Cancels<'_> {
        fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
            self.0.set(true);
            Ok(Ordering::Equal)
        }
    }
    assert_eq!(
        like(
            &text,
            &text,
            Some(LikeMode::Ansi89),
            Some(&Cancels(&cancelled)),
            &mut budget,
            Some(&|| cancelled.get())
        ),
        Err(EvalError::Cancelled)
    );
    let mut bytes = ResourceBudget::new(0, 0, 100);
    assert!(matches!(
        like(
            &text,
            &text,
            Some(LikeMode::Ansi89),
            Some(&Ordinal),
            &mut bytes,
            None
        ),
        Err(EvalError::Limit(_))
    ));
    let mut steps = ResourceBudget::new(1_000_000, 0, 100);
    assert!(matches!(
        like(
            &Value::Text("a".repeat(100)),
            &Value::Text("*a*a*a*a*a*b".into()),
            Some(LikeMode::Ansi89),
            Some(&Ordinal),
            &mut steps,
            None
        ),
        Err(EvalError::Limit(_))
    ));
}
#[test]
fn supplied_collation_controls_literal_and_range_comparisons_and_errors() {
    struct Fold;
    impl Collation for Fold {
        fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
            Ok(a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()))
        }
    }
    let mut budget = ResourceBudget::new(1_000_000, 0, 1_000_000);
    for pattern in ["a", "[A-Z]"] {
        assert_eq!(
            like(
                &Value::Text("a".into()),
                &Value::Text(pattern.into()),
                Some(LikeMode::Ansi89),
                Some(&Fold),
                &mut budget,
                None
            ),
            Ok(Truth::True)
        );
    }
    struct Fails;
    impl Collation for Fails {
        fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
            Err(EvalError::Domain)
        }
    }
    assert_eq!(
        like(
            &Value::Text("a".into()),
            &Value::Text("[a]".into()),
            Some(LikeMode::Ansi89),
            Some(&Fails),
            &mut budget,
            None
        ),
        Err(EvalError::Domain)
    );
}
// Independent exhaustive recursive model on small inputs; production uses iterative DP.
fn oracle(text: &[u8], pattern: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|offset| oracle(&text[offset..], rest)),
        Some((head, rest)) => {
            !text.is_empty() && (*head == b'?' || *head == text[0]) && oracle(&text[1..], rest)
        }
    }
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn generated_patterns_agree_with_independent_exhaustive_model(text in "[ab]{0,8}", pattern in "[ab*?]{0,8}") {
        let expected = if oracle(text.as_bytes(), pattern.as_bytes()) { Truth::True } else { Truth::False };
        prop_assert_eq!(matches(&text, &pattern, LikeMode::Ansi89), Ok(expected));
        let ansi92 = pattern.replace('*', "%").replace('?', "_");
        prop_assert_eq!(matches(&text, &ansi92, LikeMode::Ansi92), Ok(expected));
    }
}
