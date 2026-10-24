use super::raw_tests::{number, raw};
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch, StringSearchProfile},
    evaluation::{Evaluator, NoBindings},
};
use std::{cell::Cell, cmp::Ordering, ops::Range};
use tablecraft_core::limits::ResourceBudget;

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
        panic!("replacement must use consumed spans");
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
        _: &[u16],
        _: &[u16],
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        assert_eq!(search, StringSearch::ForwardFrom(0));
        assert_eq!(mode, StringCompareMode::Database);
        self.calls.set(self.calls.get() + 1);
        self.cancel_after.set(self.cancel);
        self.result.clone()
    }
}

#[test]
fn raw_replacement_refuses_invalid_nonadvancing_spans_and_callback_failure() {
    for expected in [
        Ok(Some(0..0)),
        Ok(Some(0..3)),
        Ok(Some(Range { start: 2, end: 1 })),
        Err(EvalError::CollationFailed),
    ] {
        let failure = if expected.is_err() {
            EvalError::CollationFailed
        } else {
            EvalError::InvalidTextMatch
        };
        let host = Provider::new(expected);
        let arguments = [
            raw(&[0xd800, 65]),
            raw(&[65]),
            raw(&[66]),
            number(1),
            number(-1),
            number(2),
        ];
        let mut budget = ResourceBudget::new(65536, 0, 100000);
        assert!(matches!(
            Evaluator::new(&NoBindings, &mut budget, Default::default())
                .with_collation(&host)
                .apply_retained_replacement(&arguments),
            Err(error) if error == failure
        ));
        assert_eq!(host.calls.get(), 1);
    }
}

#[test]
fn raw_replacement_checks_cancellation_after_a_successful_provider_callback() {
    let host = Provider {
        cancel: true,
        ..Provider::new(Ok(Some(0..1)))
    };
    let arguments = [
        raw(&[0xd800]),
        raw(&[0xd800]),
        raw(&[66]),
        number(1),
        number(-1),
        number(2),
    ];
    let mut budget = ResourceBudget::new(65536, 0, 100000);
    assert!(matches!(
        Evaluator::new(&NoBindings, &mut budget, Default::default())
            .with_collation(&host)
            .with_cancellation(&|| host.cancel_after.get())
            .apply_retained_replacement(&arguments),
        Err(EvalError::Cancelled)
    ));
    assert_eq!(host.calls.get(), 1);
}
