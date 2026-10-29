//! Database-mode matching does not infer a Windows Text/LCID policy.
use super::{FRENCH_LEGACY_ORDER, FrenchLegacyCollation};
use crate::collation::adapt;
use std::ops::Range;
use tablecraft_ace::{
    FrenchTextSearch, NativeSortOrder, find_french_legacy_text, find_french_legacy_utf16,
};
use tablecraft_expressions::{
    EvalError,
    comparison::{StringCompareMode, StringSearch},
};

impl FrenchLegacyCollation<'_> {
    pub(super) fn search(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        require_database(mode)?;
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        find_french_legacy_text(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            haystack,
            needle,
            bounds(search),
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }

    pub(super) fn search_utf16(
        &self,
        haystack: &[u16],
        needle: &[u16],
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        require_database(mode)?;
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        find_french_legacy_utf16(
            NativeSortOrder::from_native_bytes(FRENCH_LEGACY_ORDER),
            haystack,
            needle,
            bounds(search),
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }
}

fn require_database(mode: StringCompareMode) -> Result<(), EvalError> {
    if mode == StringCompareMode::Database {
        Ok(())
    } else {
        Err(EvalError::CollationRequired)
    }
}

fn bounds(search: StringSearch) -> FrenchTextSearch {
    match search {
        StringSearch::ForwardFrom(start) => FrenchTextSearch::ForwardFrom(start),
        StringSearch::ReverseThrough(end) => FrenchTextSearch::ReverseThrough(end),
    }
}
