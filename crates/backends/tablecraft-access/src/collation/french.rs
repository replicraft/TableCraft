//! French legacy comparison, substring and LIKE with one cumulative allowance.
use super::{adapt, budget};
use std::ops::Range;
use std::{cell::RefCell, cmp::Ordering};
use tablecraft_ace::{
    InspectError, NativeSortOrder, compare_french_legacy_text, compare_french_legacy_utf16,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{
    EvalError,
    comparison::{
        Collation, LikeProfile, LikeSpanState, StringCompareMode, StringComparisonProfile,
        StringSearch, StringSearchProfile,
    },
};

mod like;
mod search;

const FRENCH_LEGACY_ORDER: [u8; 4] = [12, 4, 0, 0];

/// Explicit provisional French legacy 1036/0/0 full-string comparison.
///
/// SQL/default and explicit Database comparison use the engineering weight model.
/// Database substring search uses complete weights and consumed UTF-16 spans.
/// LIKE uses separate untrimmed full keys and the existing UTF-16 grammar.
/// This does not implement Windows Text/LCID comparison,
/// equality-prefix hints or native Access SQL qualification. Binary operations
/// and policy-independent search controls retain their existing evaluator rules.
pub struct FrenchLegacyCollation<'a> {
    budget: budget::Budget<'a>,
    option_compare: Option<StringCompareMode>,
    cancelled: &'a dyn Fn() -> bool,
}

impl<'a> FrenchLegacyCollation<'a> {
    /// Validate exact 1036/0/0 and retain an owned cumulative comparison quota.
    pub fn new(
        order: NativeSortOrder,
        mut budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        qualify(order, &mut budget, cancelled)?;
        Ok(Self {
            budget: budget::Budget::Owned(RefCell::new(budget)),
            option_compare: None,
            cancelled,
        })
    }

    pub(super) fn new_shared(
        order: NativeSortOrder,
        budget: &'a RefCell<ResourceBudget>,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        // Resolver setup is metadata-only. The existing General construction
        // charges remain unchanged; lookup and actual French work use the same
        // shared quota when this policy is selected.
        if cancelled() {
            return Err(InspectError::Cancelled);
        }
        if order != NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER) {
            return Err(InspectError::Unsupported);
        }
        Ok(Self {
            budget: budget::Budget::Shared(budget),
            option_compare: None,
            cancelled,
        })
    }

    /// Set an explicit host default string mode without adding Text/NLS support.
    pub fn with_option_compare(mut self, mode: StringCompareMode) -> Self {
        self.option_compare = Some(mode);
        self
    }
}

impl Collation for FrenchLegacyCollation<'_> {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        compare_french_legacy_text(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            left,
            right,
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        compare_french_legacy_utf16(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            left,
            right,
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
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

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        self.compare_pattern_units(left, right)
    }

    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        self.pattern_unit(units)
    }

    fn string_search_profile(&self, mode: StringCompareMode) -> StringSearchProfile {
        if mode == StringCompareMode::Database {
            StringSearchProfile::Utf16
        } else {
            StringSearchProfile::Unicode
        }
    }

    fn find(
        &self,
        haystack: &str,
        needle: &str,
        bounds: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.search(haystack, needle, bounds, mode)
    }

    fn find_utf16(
        &self,
        haystack: &[u16],
        needle: &[u16],
        bounds: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        self.search_utf16(haystack, needle, bounds, mode)
    }

    fn option_compare(&self) -> Option<StringCompareMode> {
        self.option_compare
    }
}

fn qualify(
    order: NativeSortOrder,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    if order != NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER) {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}
