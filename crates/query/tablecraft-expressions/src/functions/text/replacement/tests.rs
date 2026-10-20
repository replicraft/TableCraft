use super::*;
use crate::functions::text::{TextFunction, TextResult, apply};
use proptest::prelude::*;
use std::{cell::Cell, cmp::Ordering};
struct Output {
    text: String,
    steps: usize,
    bytes: usize,
}
impl TextOutput for Output {
    fn step(&mut self) -> Result<(), EvalError> {
        if self.steps == 0 {
            return Err(EvalError::Cancelled);
        }
        self.steps -= 1;
        Ok(())
    }
    fn append(&mut self, fragment: &str) -> Result<(), EvalError> {
        self.step()?;
        if self.text.len() + fragment.len() > self.bytes {
            return Err(EvalError::TextLimit);
        }
        self.text.push_str(fragment);
        Ok(())
    }
}
fn text(value: &str) -> Value {
    Value::Text(value.into())
}
fn render(
    args: &[Value],
    host: Option<&dyn Collation>,
    steps: usize,
    bytes: usize,
) -> Result<String, EvalError> {
    let TextResult::Replacement(replacement) = apply(TextFunction::Replace, args, || Ok(()))?
    else {
        unreachable!()
    };
    let mut output = Output {
        text: String::new(),
        steps,
        bytes,
    };
    replacement.write(host, &mut output)?;
    Ok(output.text)
}
#[test]
fn replacement_returns_suffix_and_does_not_rescan_inserted_text_or_overlap() {
    for (source, find, replacement, start, count, expected) in [
        ("banana", "an", "X", 1, -1, "bXXa"),
        ("banana", "an", "X", 3, -1, "nXa"),
        ("banana", "an", "X", 1, 1, "bXana"),
        ("aaaa", "aa", "a", 1, -1, "aa"),
        ("a", "a", "aa", 1, -1, "aa"),
        ("banana", "an", "", 1, -1, "ba"),
        ("banana", "", "X", 3, -1, "nana"),
        ("banana", "an", "X", 3, 0, "nana"),
        ("banana", "an", "X", 20, -1, ""),
        ("", "a", "x", 1, -1, ""),
        ("😀x😀", "😀", "é", 3, -1, "xé"),
        ("a\0b\0", "\0", "X", 1, -1, "aXbX"),
        ("é e\u{301}", "é", "X", 1, -1, "X e\u{301}"),
    ] {
        let args = [
            text(source),
            text(find),
            text(replacement),
            Value::Int32(start),
            Value::Int32(count),
        ];
        assert_eq!(render(&args, None, 1000, 1000), Ok(expected.into()));
    }
}
#[test]
fn null_domain_conversion_and_surrogate_errors_stay_explicit() {
    for args in [
        vec![Value::Null, text("a"), text("b")],
        vec![text("a"), Value::Null, text("b")],
        vec![text("a"), text("b"), Value::Null],
        vec![text("a"), text("a"), text("b"), Value::Null],
        vec![text("a"), text("a"), text("b"), Value::Int16(0)],
        vec![
            text("a"),
            text("a"),
            text("b"),
            Value::Int16(1),
            Value::Int16(-2),
        ],
        vec![
            text("a"),
            text("a"),
            text("b"),
            Value::Int16(1),
            Value::Null,
        ],
        vec![
            text("a"),
            text("a"),
            text("b"),
            Value::Int16(1),
            Value::Int16(-1),
            Value::Null,
        ],
        vec![
            text("a"),
            text("a"),
            text("b"),
            Value::Int16(1),
            Value::Int16(-1),
            Value::Int16(3),
        ],
    ] {
        assert!(render(&args, None, 1000, 1000).is_err());
    }
    assert_eq!(
        render(
            &[text("😀x"), text("x"), text("y"), Value::Int16(2)],
            None,
            1000,
            1000
        ),
        Err(EvalError::UnrepresentableText)
    );
    assert_eq!(
        render(
            &[
                text("a"),
                text("a"),
                text("b"),
                Value::Int16(1),
                Value::Int16(-1),
                Value::Int16(-1)
            ],
            None,
            1000,
            1000
        ),
        Err(EvalError::CollationRequired)
    );
}
struct Host {
    calls: Cell<usize>,
    invalid: bool,
}
impl Collation for Host {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
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
        assert_eq!(needle, "SS");
        assert_eq!(search, StringSearch::ForwardFrom(0));
        assert_eq!(mode, StringCompareMode::Text);
        self.calls.set(self.calls.get() + 1);
        if self.invalid {
            return Ok(Some(0..0));
        }
        Ok(haystack.find('ß').map(|byte| {
            let start = haystack[..byte].encode_utf16().count();
            start..start + 1
        }))
    }
}
#[test]
fn locale_spans_can_change_lengths_and_nonadvancing_providers_fail() {
    let host = Host {
        calls: Cell::new(0),
        invalid: false,
    };
    let args = [
        text("Straße Straße"),
        text("SS"),
        text("X"),
        Value::Int16(1),
        Value::Int16(-1),
        Value::Int16(-1),
    ];
    assert_eq!(
        render(&args, Some(&host), 1000, 1000),
        Ok("StraXe StraXe".into())
    );
    assert_eq!(host.calls.get(), 3);
    let bad = Host {
        calls: Cell::new(0),
        invalid: true,
    };
    assert_eq!(
        render(&args, Some(&bad), 1000, 1000),
        Err(EvalError::InvalidTextMatch)
    );
    assert_eq!(bad.calls.get(), 1);
}
#[test]
fn expansion_limits_and_cancelled_scans_fail_before_unbounded_output() {
    let args = [text("aaaa"), text("a"), text("xyz")];
    assert_eq!(render(&args, None, 1000, 5), Err(EvalError::TextLimit));
    assert_eq!(render(&args, None, 5, 1000), Err(EvalError::Cancelled));
    let host = Host {
        calls: Cell::new(0),
        invalid: false,
    };
    let args = [
        text("Straße"),
        text("SS"),
        text("x"),
        Value::Int16(1),
        Value::Int16(-1),
        Value::Int16(1),
    ];
    assert_eq!(
        render(&args, Some(&host), 2, 1000),
        Err(EvalError::Cancelled)
    );
    assert_eq!(host.calls.get(), 0);
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn binary_replacement_matches_independent_unicode_replacen(source in "[abc😀]{0,24}", needle in "[abc😀]{1,3}", replacement in ".{0,5}", start in 0usize..30, count in 0usize..8) {
        let byte=source.char_indices().nth(start).map_or(source.len(),|(byte,_)|byte);
        let units=source[..byte].encode_utf16().count();
        let args=[text(&source),text(&needle),text(&replacement),Value::Int32((units+1) as i32),Value::Int32(count as i32)];
        let expected=source[byte..].replacen(&needle,&replacement,count);
        prop_assert_eq!(render(&args,None,10000,1000),Ok(expected));
    }
}
