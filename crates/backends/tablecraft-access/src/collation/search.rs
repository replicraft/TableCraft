use super::{GeneralCollation, adapt};
use std::ops::Range;
use tablecraft_ace::{GeneralTextSearch, find_general_text, find_general_utf16};
use tablecraft_expressions::{
    EvalError,
    comparison::{StringCompareMode, StringSearch},
};

impl GeneralCollation<'_> {
    pub(super) fn search(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
        if mode != StringCompareMode::Database {
            return Err(EvalError::CollationRequired);
        }
        let search = search_bounds(search);
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        find_general_text(
            self.order,
            haystack,
            needle,
            search,
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
        if mode != StringCompareMode::Database {
            return Err(EvalError::CollationRequired);
        }
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| EvalError::CollationFailed)?;
        find_general_utf16(
            self.order,
            haystack,
            needle,
            search_bounds(search),
            &mut budget,
            self.cancelled,
        )
        .map_err(adapt)
    }
}

fn search_bounds(search: StringSearch) -> GeneralTextSearch {
    match search {
        StringSearch::ForwardFrom(start) => GeneralTextSearch::ForwardFrom(start),
        StringSearch::ReverseThrough(end) => GeneralTextSearch::ReverseThrough(end),
    }
}
