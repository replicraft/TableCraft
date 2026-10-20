//! Execution policies for text functions, independent of native file metadata.
use super::code_page::CharacterCodePage;
use crate::comparison::Collation;

/// Caller-selected text semantics; no ambient locale or machine setting is read.
#[derive(Clone, Copy, Default)]
pub struct TextContext<'a> {
    /// Explicit numeric output syntax for CStr; other functions retain their policies.
    pub number_format: Option<crate::functions::conversions::text::NumberFormat>,
    /// Explicit CStr date/time output syntax, independent of numeric formatting.
    pub date_format: Option<super::cstr::DateTextFormat>,
    /// Distinct database/text comparison and search provider, if supplied.
    pub collation: Option<&'a dyn Collation>,
    /// Explicit numeric-byte character decoding policy; absent keeps ASCII-only.
    pub character_code_page: Option<CharacterCodePage>,
}
