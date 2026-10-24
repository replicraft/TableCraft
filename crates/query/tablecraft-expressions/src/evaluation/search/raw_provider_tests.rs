use crate::{
    EvalError,
    comparison::{
        Collation, StringCompareMode, StringComparisonProfile, StringSearch, StringSearchProfile,
    },
    evaluation::{Evaluator, NoBindings},
    functions::text::TextFunction,
};
use std::{cell::Cell, cmp::Ordering, ops::Range};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionString, ExpressionValue, Value},
};

struct Provider {
    result: Result<Option<Range<usize>>, EvalError>,
    calls: Cell<usize>,
    cancel_after: Cell<bool>,
    cancel: bool,
}

impl Provider {
    fn new(result: Result<Option<Range<usize>>, EvalError>) -> Self {
        Self {
            result,
            calls: Cell::new(0),
            cancel_after: Cell::new(false),
            cancel: false,
        }
    }
}

impl Collation for Provider {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        panic!("substring provider must not use equality")
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }

    fn string_search_profile(&self, mode: StringCompareMode) -> StringSearchProfile {
        if mode == StringCompareMode::Database {
            StringSearchProfile::Utf16
        } else {
            StringSearchProfile::Unicode
        }
    }

    fn find_utf16(
        &self,
        haystack: &[u16],
        needle: &[u16],
        _: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        assert_eq!(haystack, [0xd800, 65, 66]);
        assert_eq!(needle, [65, 66]);
        assert_eq!(mode, StringCompareMode::Database);
        self.calls.set(self.calls.get() + 1);
        self.cancel_after.set(self.cancel);
        self.result.clone()
    }
}

fn raw(units: &[u16]) -> ExpressionValue {
    ExpressionValue::StringBytes(ExpressionString::from_owned(
        units.iter().flat_map(|unit| unit.to_le_bytes()).collect(),
    ))
}

fn number(value: i32) -> ExpressionValue {
    ExpressionValue::Value(Value::Int32(value))
}

fn arguments(reverse: bool, start: i32, mode: i32) -> (TextFunction, Vec<ExpressionValue>) {
    if reverse {
        (
            TextFunction::InStrRev,
            vec![
                raw(&[0xd800, 65, 66]),
                raw(&[65, 66]),
                number(start),
                number(mode),
            ],
        )
    } else {
        (
            TextFunction::InStr,
            vec![
                number(start),
                raw(&[0xd800, 65, 66]),
                raw(&[65, 66]),
                number(mode),
            ],
        )
    }
}

#[test]
fn raw_search_contract_is_separate_from_equality_like_and_system_text() {
    let host = Provider::new(Ok(Some(1..3)));
    assert_eq!(
        host.string_comparison_profile(),
        StringComparisonProfile::Unicode
    );
    for (reverse, start, mode) in [(false, 1, 2), (false, 2, -1), (true, -1, 2)] {
        let (function, arguments) = arguments(reverse, start, mode);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_collation(&host)
                .apply_expression_search(function, &arguments),
            Ok(Value::Int32(2)),
        );
    }
    assert_eq!(host.calls.get(), 3);

    let (function, arguments) = arguments(false, 1, 1);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&host)
            .apply_expression_search(function, &arguments),
        Err(EvalError::UnrepresentableText),
    );
    assert_eq!(host.calls.get(), 3);
}

#[test]
fn malformed_provider_spans_and_callback_failures_never_publish_a_position() {
    for (reverse, start, range) in [
        (false, 1, 4..5),
        (false, 3, 1..3),
        (true, 2, 1..3),
        (false, 1, 1..4),
        (false, 1, Range { start: 2, end: 1 }),
    ] {
        let host = Provider::new(Ok(Some(range)));
        let (function, arguments) = arguments(reverse, start, 2);
        let mut budget = ResourceBudget::new(65536, 0, 10000);
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_collation(&host)
                .apply_expression_search(function, &arguments),
            Err(EvalError::InvalidTextMatch),
        );
        assert_eq!(host.calls.get(), 1);
    }
    let host = Provider::new(Err(EvalError::CollationFailed));
    let (function, arguments) = arguments(false, 1, 2);
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&host)
            .apply_expression_search(function, &arguments),
        Err(EvalError::CollationFailed),
    );
    assert_eq!(host.calls.get(), 1);
}

#[test]
fn raw_provider_search_charges_conversion_and_checks_after_successful_callback() {
    let host = Provider::new(Ok(Some(1..3)));
    let (function, arguments) = arguments(false, 1, 2);
    let mut budget = ResourceBudget::new(0, 0, 10000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&host)
            .apply_expression_search(function, &arguments),
        Err(EvalError::Limit(error)) if error.resource == Resource::Bytes
    ));
    assert_eq!(host.calls.get(), 0);

    let host = Provider {
        cancel: true,
        ..Provider::new(Ok(Some(1..3)))
    };
    let mut budget = ResourceBudget::new(65536, 0, 10000);
    assert_eq!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&host)
            .with_cancellation(&|| host.cancel_after.get())
            .apply_expression_search(function, &arguments),
        Err(EvalError::Cancelled),
    );
    assert_eq!(host.calls.get(), 1);
}
