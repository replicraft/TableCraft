/// A parsed identifier, preserving spelling without upstream token types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(pub(crate) String);

impl Name {
    /// Identifier contents without delimiters; no collation or case folding.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A field optionally qualified by a table name or alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldReference {
    /// Optional qualifier, without name resolution.
    pub qualifier: Option<Name>,
    /// Field name or unqualified parameter prompt; binding resolves its meaning.
    pub name: Name,
}
