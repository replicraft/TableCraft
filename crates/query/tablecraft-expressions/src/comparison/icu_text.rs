//! Explicit ICU host text policy, independent of native database/NLS qualification.
mod locale;
mod search;

pub use locale::IcuTextLocale;

use super::{
    Collation, LikeProfile, LikeSpanState, LocaleId, StringCompareMode, StringComparisonProfile,
    StringSearch, StringSearchProfile,
};
use crate::EvalError;
use icu_collator::{
    Collator, CollatorBorrowed,
    options::{AlternateHandling, CaseLevel, CollatorOptions, Strength},
};
use std::{cell::RefCell, cmp::Ordering, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

/// Validated ceilings for atomic ICU calls and exhaustive substring enumeration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IcuTextLimits {
    maximum_input_bytes: usize,
    maximum_search_units: usize,
}

impl IcuTextLimits {
    /// Maximum bytes in each operand of an indivisible ICU comparison.
    pub const MAX_INPUT_BYTES: usize = 65_536;
    /// Maximum UTF-16 units in either substring-search operand.
    pub const MAX_SEARCH_UNITS: usize = 4096;

    /// Choose positive ceilings without enlarging the bounded host profile.
    pub fn new(maximum_input_bytes: usize, maximum_search_units: usize) -> Result<Self, EvalError> {
        if maximum_input_bytes == 0
            || maximum_input_bytes > Self::MAX_INPUT_BYTES
            || maximum_search_units == 0
            || maximum_search_units > Self::MAX_SEARCH_UNITS
        {
            return Err(EvalError::Domain);
        }
        Ok(Self {
            maximum_input_bytes,
            maximum_search_units,
        })
    }
}

impl Default for IcuTextLimits {
    fn default() -> Self {
        Self {
            maximum_input_bytes: Self::MAX_INPUT_BYTES,
            maximum_search_units: Self::MAX_SEARCH_UNITS,
        }
    }
}

/// A bounded ICU text-mode provider composed over an optional database policy.
/// Secondary strength ignores case but retains accents; punctuation is nonignorable.
/// An LCID dispatch is enabled only by explicit host registration. Neither its
/// registration nor ICU locale data establishes Windows NLS/Access equivalence.
/// Provider quotas accumulate across calls; ICU's internal scratch allocator is
/// bounded by input ceilings but is not a fallible caller-owned allocator.
pub struct IcuTextCollation<'a> {
    collator: CollatorBorrowed<'static>,
    database: Option<&'a dyn Collation>,
    registered: Option<LocaleId>,
    budget: RefCell<ResourceBudget>,
    cancelled: &'a dyn Fn() -> bool,
    limits: IcuTextLimits,
}

impl<'a> IcuTextCollation<'a> {
    /// Construct a provider from compiled ICU data without inspecting a database.
    pub fn new(
        locale: IcuTextLocale,
        budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, EvalError> {
        if cancelled() {
            return Err(EvalError::Cancelled);
        }
        let preferences = locale.preferences();
        let mut options = CollatorOptions::default();
        options.strength = Some(Strength::Secondary);
        options.case_level = Some(CaseLevel::Off);
        options.alternate_handling = Some(AlternateHandling::NonIgnorable);
        let collator =
            Collator::try_new(preferences, options).map_err(|_| EvalError::CollationFailed)?;
        if cancelled() {
            return Err(EvalError::Cancelled);
        }
        Ok(Self {
            collator,
            database: None,
            registered: None,
            budget: RefCell::new(budget),
            cancelled,
            limits: IcuTextLimits::default(),
        })
    }

    /// Delegate ordinary SQL/database, raw-unit and LIKE policies without alteration.
    pub fn with_database(mut self, database: &'a dyn Collation) -> Self {
        self.database = Some(database);
        self
    }

    /// Explicitly register this profile for one complete host-issued LCID.
    /// No language number or alternate-sort alias is inferred from the locale.
    pub fn with_registered_locale(mut self, locale: LocaleId) -> Self {
        self.registered = Some(locale);
        self
    }

    /// Lower operation ceilings without resetting consumed provider work.
    pub fn with_limits(mut self, limits: IcuTextLimits) -> Self {
        self.limits = limits;
        self
    }

    fn base(&self) -> Result<&dyn Collation, EvalError> {
        self.database.ok_or(EvalError::CollationRequired)
    }

    fn check(&self, budget: &mut ResourceBudget, steps: usize) -> Result<(), EvalError> {
        if (self.cancelled)() {
            return Err(EvalError::Cancelled);
        }
        budget.charge(Resource::Steps, steps as u64)?;
        Ok(())
    }

    fn compare_checked(
        &self,
        left: &str,
        right: &str,
        budget: &mut ResourceBudget,
    ) -> Result<Ordering, EvalError> {
        self.check(budget, 1)?;
        if left.len() > self.limits.maximum_input_bytes
            || right.len() > self.limits.maximum_input_bytes
        {
            return Err(EvalError::TextLimit);
        }
        // Prepay byte traversal before each indivisible bounded ICU call. This
        // is an admission model, not an exact count of ICU collation elements.
        self.check(budget, left.len() + right.len())?;
        let result = self.collator.compare(left, right);
        self.check(budget, 1)?;
        Ok(result)
    }
}

impl Collation for IcuTextCollation<'_> {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        self.base()?.compare(left, right)
    }
    fn compare_text(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        if (self.cancelled)() {
            return Err(EvalError::Cancelled);
        }
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        self.compare_checked(left, right, &mut budget)
    }
    fn find(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        if mode == StringCompareMode::Text {
            self.search(haystack, needle, search)
        } else {
            self.base()?.find(haystack, needle, search, mode)
        }
    }
    fn find_locale(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        locale: LocaleId,
    ) -> Result<Option<Range<usize>>, EvalError> {
        if self.registered != Some(locale) {
            return Err(EvalError::CollationRequired);
        }
        self.search(haystack, needle, search)
    }
    fn option_compare(&self) -> Option<StringCompareMode> {
        self.database.and_then(Collation::option_compare)
    }
    fn string_comparison_profile(&self) -> StringComparisonProfile {
        self.database.map_or(
            StringComparisonProfile::Unicode,
            Collation::string_comparison_profile,
        )
    }
    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        self.base()?.compare_utf16(left, right)
    }
    fn like_profile(&self) -> LikeProfile {
        self.database
            .map_or(LikeProfile::UnicodeScalar, Collation::like_profile)
    }
    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        self.base()?.compare_like_units(left, right)
    }
    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        self.base()?.is_like_unit(units)
    }
    fn like_literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        self.base()?.like_literal_step(candidate, literal)
    }
    fn like_unit_step(&self, candidate: &[u16]) -> Result<LikeSpanState, EvalError> {
        self.base()?.like_unit_step(candidate)
    }
    fn string_search_profile(&self, mode: StringCompareMode) -> StringSearchProfile {
        if mode == StringCompareMode::Text {
            StringSearchProfile::Unicode
        } else {
            self.database.map_or(StringSearchProfile::Unicode, |base| {
                base.string_search_profile(mode)
            })
        }
    }
    fn find_utf16(
        &self,
        haystack: &[u16],
        needle: &[u16],
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        if mode == StringCompareMode::Text {
            return Err(EvalError::UnsupportedExpression);
        }
        self.base()?.find_utf16(haystack, needle, search, mode)
    }
}

#[cfg(test)]
mod expanded_tests;
#[cfg(test)]
mod tests;
