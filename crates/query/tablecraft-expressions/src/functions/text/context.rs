//! Execution policies for text functions, independent of native file metadata.
use super::code_page::CharacterCodePage;
use crate::comparison::Collation;

/// Caller-selected text semantics; no ambient locale or machine setting is read.
#[derive(Clone, Copy, Default)]
pub struct TextContext<'a> {
    /// Distinct database/text comparison and search provider, if supplied.
    pub collation: Option<&'a dyn Collation>,
    /// Explicit numeric-byte character decoding policy; absent keeps ASCII-only.
    pub character_code_page: Option<CharacterCodePage>,
}
