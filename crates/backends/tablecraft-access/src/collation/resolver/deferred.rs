//! Refusing default provider for an exact unsupported stored descriptor.
use std::{cell::RefCell, cmp::Ordering, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile, LocaleId, StringCompareMode, StringSearch},
};

pub(super) struct DeferredDefault<'a> {
    budget: &'a RefCell<ResourceBudget>,
    cancelled: &'a dyn Fn() -> bool,
    option_compare: Option<StringCompareMode>,
}

impl<'a> DeferredDefault<'a> {
    pub(super) fn new(
        budget: &'a RefCell<ResourceBudget>,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            budget,
            cancelled,
            option_compare: None,
        }
    }

    pub(super) fn with_option_compare(mut self, mode: StringCompareMode) -> Self {
        self.option_compare = Some(mode);
        self
    }

    fn refuse<T>(&self) -> Result<T, EvalError> {
        if (self.cancelled)() {
            return Err(EvalError::Cancelled);
        }
        self.budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?
            .charge(Resource::Steps, 1)?;
        Err(EvalError::CollationRequired)
    }
}

impl Collation for DeferredDefault<'_> {
    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Unsupported
    }

    fn compare(&self, _left: &str, _right: &str) -> Result<Ordering, EvalError> {
        self.refuse()
    }

    fn compare_utf16(&self, _left: &[u16], _right: &[u16]) -> Result<Ordering, EvalError> {
        self.refuse()
    }

    fn compare_like_units(&self, _left: &[u16], _right: &[u16]) -> Result<Ordering, EvalError> {
        self.refuse()
    }

    fn is_like_unit(&self, _units: &[u16]) -> Result<bool, EvalError> {
        self.refuse()
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        self.option_compare
    }

    fn find(
        &self,
        _haystack: &str,
        _needle: &str,
        _search: StringSearch,
        _mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.refuse()
    }

    fn find_utf16(
        &self,
        _haystack: &[u16],
        _needle: &[u16],
        _search: StringSearch,
        _mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.refuse()
    }

    fn find_locale(
        &self,
        _haystack: &str,
        _needle: &str,
        _search: StringSearch,
        _locale: LocaleId,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.refuse()
    }

    fn compare_text(&self, _left: &str, _right: &str) -> Result<Ordering, EvalError> {
        self.refuse()
    }
}
