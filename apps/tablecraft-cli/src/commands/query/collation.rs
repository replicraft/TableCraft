//! Explicit CLI comparison policies; native descriptor selection is opt-in.
use super::comparison_limits::Args as ComparisonArgs;
use clap::ValueEnum;
use std::{cmp::Ordering, ops::Range};
use tablecraft_access::GeneralCollation;
use tablecraft_ace::{InspectError, NativeSortOrder};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::{
    EvalError,
    comparison::{
        Collation, LikeProfile, LikeSpanState, StringCompareMode, StringComparisonProfile,
        StringSearch, StringSearchProfile,
    },
};

const GENERAL_LEGACY_ORDER: [u8; 4] = [9, 4, 0, 0];
const GENERAL_2010_ORDER: [u8; 4] = [9, 4, 0, 1];

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum TextCollation {
    /// Stored General database/default and per-field policies; unresolved conflicts fail.
    Database,
    /// Unicode ordinal order, case-sensitive; no normalization or locale rules.
    Ordinal,
    /// Provisional full-string General legacy weights; native SQL qualification pending.
    GeneralLegacy,
    /// Provisional full-string General 2010 weights; native SQL qualification pending.
    #[value(name = "general-2010")]
    General2010,
}
#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum UpdateCollation {
    /// Unicode ordinal order; provisional General policies are read/query-only.
    Ordinal,
}
impl From<UpdateCollation> for TextCollation {
    fn from(policy: UpdateCollation) -> Self {
        match policy {
            UpdateCollation::Ordinal => Self::Ordinal,
        }
    }
}
impl TextCollation {
    pub(super) fn build(self) -> Result<SelectedCollation, InspectError> {
        self.build_with_budget(comparison_budget())
    }

    pub(super) fn build_with_budget(
        self,
        budget: ResourceBudget,
    ) -> Result<SelectedCollation, InspectError> {
        let order = match self {
            Self::Database => return Err(InspectError::Unsupported),
            Self::Ordinal => return Ok(SelectedCollation::Ordinal),
            Self::GeneralLegacy => GENERAL_LEGACY_ORDER,
            Self::General2010 => GENERAL_2010_ORDER,
        };
        Ok(SelectedCollation::General(GeneralCollation::new(
            NativeSortOrder::from_native_bytes(order),
            budget,
            &|| false,
        )?))
    }
}
pub(super) fn comparison_budget() -> ResourceBudget {
    ComparisonArgs::default().budget()
}
pub(super) enum SelectedCollation {
    Ordinal,
    General(GeneralCollation<'static>),
}
impl Collation for SelectedCollation {
    fn string_comparison_profile(&self) -> StringComparisonProfile {
        match self {
            Self::Ordinal => StringComparisonProfile::Unicode,
            Self::General(policy) => policy.string_comparison_profile(),
        }
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        match self {
            Self::Ordinal => Err(EvalError::UnsupportedExpression),
            Self::General(policy) => policy.compare_utf16(left, right),
        }
    }

    fn like_literal_step(
        &self,
        candidate: &[u16],
        literal: &[u16],
    ) -> Result<LikeSpanState, EvalError> {
        match self {
            Self::Ordinal => Err(EvalError::UnsupportedExpression),
            Self::General(policy) => policy.like_literal_step(candidate, literal),
        }
    }
    fn like_unit_step(&self, candidate: &[u16]) -> Result<LikeSpanState, EvalError> {
        match self {
            Self::Ordinal => Err(EvalError::UnsupportedExpression),
            Self::General(policy) => policy.like_unit_step(candidate),
        }
    }
    fn like_profile(&self) -> LikeProfile {
        match self {
            Self::Ordinal => LikeProfile::UnicodeScalar,
            Self::General(policy) => policy.like_profile(),
        }
    }
    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        match self {
            Self::Ordinal => Err(EvalError::UnsupportedExpression),
            Self::General(policy) => policy.compare_like_units(left, right),
        }
    }
    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        match self {
            Self::Ordinal => Err(EvalError::UnsupportedExpression),
            Self::General(policy) => policy.is_like_unit(units),
        }
    }
    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        match self {
            Self::Ordinal => Ok(left.cmp(right)),
            Self::General(policy) => policy.compare(left, right),
        }
    }
    fn string_search_profile(&self, mode: StringCompareMode) -> StringSearchProfile {
        match self {
            Self::Ordinal => StringSearchProfile::Unicode,
            Self::General(policy) => policy.string_search_profile(mode),
        }
    }

    fn find_utf16(
        &self,
        haystack: &[u16],
        needle: &[u16],
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        match self {
            Self::Ordinal => Err(EvalError::UnsupportedExpression),
            Self::General(policy) => policy.find_utf16(haystack, needle, search, mode),
        }
    }

    fn find(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        if let Self::General(policy) = self {
            return policy.find(haystack, needle, search, mode);
        }
        if mode != StringCompareMode::Database {
            return Err(EvalError::CollationRequired);
        }
        // Host callbacks are bounded by the caller's already validated text ceiling.
        // Round a boundary inside a surrogate pair toward the searched region:
        // a valid Unicode needle cannot begin/end at the other half of that pair.
        let mut byte = haystack.len();
        let mut units = 0;
        for (offset, character) in haystack.char_indices() {
            match search {
                StringSearch::ForwardFrom(start) if units >= start => {
                    byte = offset;
                    break;
                }
                StringSearch::ReverseThrough(end) if units + character.len_utf16() > end => {
                    byte = offset;
                    break;
                }
                _ => {}
            }
            units += character.len_utf16();
        }
        let found = match search {
            StringSearch::ForwardFrom(_) => {
                haystack[byte..].find(needle).map(|offset| byte + offset)
            }
            StringSearch::ReverseThrough(_) => haystack[..byte].rfind(needle),
        };
        Ok(found.map(|byte| {
            let start = haystack[..byte].encode_utf16().count();
            start..start + needle.encode_utf16().count()
        }))
    }
}
