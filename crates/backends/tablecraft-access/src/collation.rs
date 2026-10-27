//! Explicit bridge from standalone General weights to supplied query comparison.
mod budget;
mod french;
pub use french::FrenchLegacyCollation;
mod resolver;
pub use resolver::NativeCollationResolver;
pub(crate) use resolver::identity;
mod like;
mod search;
use std::{cell::RefCell, cmp::Ordering};
use tablecraft_ace::{InspectError, NativeSortOrder, compare_general_text, compare_general_utf16};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{
    EvalError,
    comparison::{
        Collation, LikeProfile, LikeSpanState, StringCompareMode, StringComparisonProfile,
        StringSearch, StringSearchProfile,
    },
};

/// Caller-selected provisional General comparison with one cumulative budget.
///
/// No database/field locale is inferred. This applies the standalone engine's
/// full-string weight model, not qualified native Access SQL semantics. Queries
/// must keep their original filters; this policy does not enable index pushdown.
/// The caller supplies cancellation and module/text-mode/LIKE policies separately.
/// Database-mode search uses the explicit full-weight consumed-span model.
/// No captured SQL, text values or sort keys enter tracing.
pub struct GeneralCollation<'a> {
    order: NativeSortOrder,
    budget: budget::Budget<'a>,
    option_compare: Option<StringCompareMode>,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> GeneralCollation<'a> {
    fn new_shared(
        order: NativeSortOrder,
        budget: &'a RefCell<ResourceBudget>,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?
            .charge(Resource::Steps, 1)?;
        order.general_family().ok_or(InspectError::Unsupported)?;
        Ok(Self {
            order,
            budget: budget::Budget::Shared(budget),
            option_compare: None,
            cancelled,
        })
    }
    /// Set an explicit host Option Compare policy for string built-ins.
    /// This does not implement a separate textual/locale comparison provider.
    pub fn with_option_compare(mut self, mode: StringCompareMode) -> Self {
        self.option_compare = Some(mode);
        self
    }

    /// Validate the descriptor and retain the caller-issued operation budget.
    pub fn new(
        order: NativeSortOrder,
        mut budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        order.general_family().ok_or(InspectError::Unsupported)?;
        Ok(Self {
            order,
            budget: budget::Budget::Owned(RefCell::new(budget)),
            option_compare: None,
            cancelled,
        })
    }
}

impl Collation for GeneralCollation<'_> {
    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        compare_general_utf16(self.order, left, right, &mut budget, self.cancelled).map_err(adapt)
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        self.option_compare
    }
    fn like_literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        self.literal_step(candidate, literal)
    }
    fn like_unit_step(&self, candidate: &[u16]) -> Result<LikeSpanState, EvalError> {
        self.unit_step(candidate)
    }
    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }
    fn compare_like_units(
        &self,
        left: &[u16],
        right: &[u16],
    ) -> Result<std::cmp::Ordering, EvalError> {
        self.compare_pattern_units(left, right)
    }
    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        self.pattern_unit(units)
    }
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        compare_general_text(self.order, left, right, &mut budget, self.cancelled).map_err(adapt)
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
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<std::ops::Range<usize>>, EvalError> {
        self.search_utf16(haystack, needle, search, mode)
    }

    fn find(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<std::ops::Range<usize>>, EvalError> {
        self.search(haystack, needle, search, mode)
    }
}

fn adapt(error: InspectError) -> EvalError {
    match error {
        InspectError::Limit(error) => EvalError::Limit(error),
        InspectError::Cancelled => EvalError::Cancelled,
        InspectError::Allocation => EvalError::Allocation,
        InspectError::StructureLimit => EvalError::TextLimit,
        _ => EvalError::CollationFailed,
    }
}
