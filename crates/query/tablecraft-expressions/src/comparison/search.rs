//! Explicit substring bounds shared by string-search providers.
/// UTF-16 search bounds; positions are zero-based, independent of UTF-8 offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringSearch {
    /// Find the first match starting at or after this UTF-16 offset.
    ForwardFrom(usize),
    /// Find the last match ending at or before this exclusive UTF-16 boundary.
    ReverseThrough(usize),
}

/// Input representation explicitly supported for a selected substring mode.
/// Search is separate from equality and LIKE: expansions need consumed spans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringSearchProfile {
    /// Require decoded Unicode Text before invoking the existing search callback.
    Unicode,
    /// Accept complete UTF-16 units, including isolated surrogates.
    Utf16,
}
