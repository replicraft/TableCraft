//! Explicit CLI comparison policies; no implicit database/OS locale selection.
use clap::ValueEnum;
use std::{cmp::Ordering, ops::Range};
use tablecraft_access::GeneralCollation;
use tablecraft_ace::{InspectError, NativeSortOrder};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
};

const GENERAL_LEGACY_ORDER: [u8; 4] = [9, 4, 0, 0];
const GENERAL_2010_ORDER: [u8; 4] = [9, 4, 0, 1];
const COMPARISON_BYTES: u64 = 16 * 1024 * 1024;
const COMPARISON_STEPS: u64 = 1_000_000;

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum TextCollation {
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
        let order = match self {
            Self::Ordinal => return Ok(SelectedCollation::Ordinal),
            Self::GeneralLegacy => GENERAL_LEGACY_ORDER,
            Self::General2010 => GENERAL_2010_ORDER,
        };
        Ok(SelectedCollation::General(GeneralCollation::new(
            NativeSortOrder::from_native_bytes(order),
            ResourceBudget::new(COMPARISON_BYTES, 0, COMPARISON_STEPS),
            &|| false,
        )?))
    }
}
pub(super) enum SelectedCollation {
    Ordinal,
    General(GeneralCollation<'static>),
}
impl Collation for SelectedCollation {
    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        match self {
            Self::Ordinal => Ok(left.cmp(right)),
            Self::General(policy) => policy.compare(left, right),
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
