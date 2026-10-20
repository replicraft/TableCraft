//! Explicit CLI ordinal policy, separate from native Access locale semantics.
use clap::ValueEnum;
use std::{cmp::Ordering, ops::Range};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringCompareMode, StringSearch},
};

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum TextCollation {
    /// Unicode ordinal order, case-sensitive; no normalization or locale rules.
    Ordinal,
}
impl Collation for TextCollation {
    fn option_compare(&self) -> Option<StringCompareMode> {
        Some(StringCompareMode::Database)
    }
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
    fn find(
        &self,
        haystack: &str,
        needle: &str,
        search: StringSearch,
        mode: StringCompareMode,
    ) -> Result<Option<Range<usize>>, EvalError> {
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
