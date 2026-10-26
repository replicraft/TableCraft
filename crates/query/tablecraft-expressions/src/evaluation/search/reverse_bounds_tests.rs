use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch, StringSearchProfile},
    evaluation::{Evaluator, NoBindings},
    functions::text::TextFunction,
};
use std::{cell::Cell, cmp::Ordering, ops::Range};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{ExpressionString, ExpressionValue, Value},
};

struct Provider<'a> {
    result: Result<Option<Range<usize>>, EvalError>,
    calls: Cell<usize>,
    cancel_after: Option<&'a Cell<bool>>,
}

impl Collation for Provider<'_> {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("raw search must not use decoded equality")
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }

    fn string_search_profile(&self, _: StringCompareMode) -> StringSearchProfile {
        StringSearchProfile::Utf16
    }

    fn find_utf16(
        &self,
        haystack: &[u16],
        needle: &[u16],
        _: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        assert_eq!(haystack, [0xd800, 65, 66, 67]);
        assert_eq!(needle, [65]);
        assert!(matches!(
            mode,
            StringCompareMode::Text | StringCompareMode::Database
        ));
        self.calls.set(self.calls.get() + 1);
        if let Some(cancelled) = self.cancel_after {
            cancelled.set(true);
        }
        self.result.clone()
    }
}

fn raw(units: &[u16]) -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(
        units.iter().flat_map(|unit| unit.to_le_bytes()).collect(),
    ))
}

fn call(
    reverse: bool,
    mode: i16,
    provider: &Provider<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EvalError> {
    let start = ExpressionValue::Value(Value::Int16(2));
    let mode = ExpressionValue::Value(Value::Int16(mode));
    let source = raw(&[0xd800, 65, 66, 67]);
    let needle = raw(&[65]);
    let (function, arguments) = if reverse {
        (TextFunction::InStrRev, vec![source, needle, start, mode])
    } else {
        (TextFunction::InStr, vec![start, source, needle, mode])
    };
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    Evaluator::new(&NoBindings, &mut budget, Default::default())
        .with_collation(provider)
        .with_cancellation(cancelled)
        .apply_expression_search(function, &arguments)
}

#[test]
fn raw_callback_bounds_match_unicode_without_banning_forward_or_interior_empty_spans() {
    for mode in [-1, 1, 2] {
        for (reverse, range, expected) in [
            (true, 0..0, Ok(Value::Int32(1))),
            (true, 1..1, Ok(Value::Int32(2))),
            (true, 1..2, Ok(Value::Int32(2))),
            (true, 2..2, Err(EvalError::InvalidTextMatch)),
            (false, 2..2, Ok(Value::Int32(3))),
        ] {
            let provider = Provider {
                result: Ok(Some(range)),
                calls: Cell::new(0),
                cancel_after: None,
            };
            assert_eq!(call(reverse, mode, &provider, &|| false), expected);
            assert_eq!(provider.calls.get(), 1);
        }
    }
    let provider = Provider {
        result: Err(EvalError::CollationFailed),
        calls: Cell::new(0),
        cancel_after: None,
    };
    assert_eq!(call(true, 0, &provider, &|| false), Ok(Value::Int32(2)));
    assert_eq!(provider.calls.get(), 0);
}

#[test]
fn raw_callback_errors_and_post_callback_cancellation_precede_span_validation() {
    for mode in [-1, 1, 2] {
        for error in [EvalError::Cancelled, EvalError::CollationFailed] {
            let provider = Provider {
                result: Err(error),
                calls: Cell::new(0),
                cancel_after: None,
            };
            assert_eq!(call(true, mode, &provider, &|| false), Err(error));
            assert_eq!(provider.calls.get(), 1);
        }
        let cancelled = Cell::new(false);
        let provider = Provider {
            result: Ok(Some(2..2)),
            calls: Cell::new(0),
            cancel_after: Some(&cancelled),
        };
        assert_eq!(
            call(true, mode, &provider, &|| cancelled.get()),
            Err(EvalError::Cancelled)
        );
        assert_eq!(provider.calls.get(), 1);
    }
}
