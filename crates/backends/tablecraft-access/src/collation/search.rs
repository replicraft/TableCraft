use super::{GeneralCollation, adapt};
use std::ops::Range;
use tablecraft_ace::{GeneralTextSearch, find_general_text};
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
        let search = match search {
            StringSearch::ForwardFrom(start) => GeneralTextSearch::ForwardFrom(start),
            StringSearch::ReverseThrough(end) => GeneralTextSearch::ReverseThrough(end),
        };
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
}
