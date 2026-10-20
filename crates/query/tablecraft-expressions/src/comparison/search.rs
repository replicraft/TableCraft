//! Explicit substring bounds shared by string-search providers.
/// UTF-16 search bounds; positions are zero-based, independent of UTF-8 offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringSearch {
    /// Find the first match starting at or after this UTF-16 offset.
    ForwardFrom(usize),
    /// Find the last match ending at or before this exclusive UTF-16 boundary.
    ReverseThrough(usize),
}
