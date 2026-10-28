use crate::{ParseError, ParseErrorKind, ParseLimits, SourceLocation};

/// An owned identifier, preserving spelling without upstream token types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name(pub(crate) String);

impl Name {
    /// Own one already-delimited identifier's contents without parsing SQL.
    ///
    /// Contents preserve spelling, spaces and prompt punctuation. Empty names,
    /// bracket delimiters and controls are refused. This does not authorize or
    /// resolve a database object; consumers retain their metadata policies.
    pub fn from_contents(value: &str, limits: ParseLimits) -> Result<Self, ParseError> {
        let error = |kind, message| ParseError {
            kind,
            message,
            location: SourceLocation { line: 1, column: 1 },
        };

        if value.len() > limits.max_bytes.min(ParseLimits::MAX_BYTES) || limits.max_tokens == 0 {
            return Err(error(ParseErrorKind::Limit, "identifier limit exceeded"));
        }
        if value.is_empty()
            || value.chars().any(|ch| "[]".contains(ch))
            || value.chars().any(char::is_control)
        {
            return Err(error(ParseErrorKind::Syntax, "invalid identifier contents"));
        }

        let mut owned = String::new();
        owned
            .try_reserve_exact(value.len())
            .map_err(|_| error(ParseErrorKind::Limit, "identifier allocation failed"))?;
        owned.push_str(value);

        Ok(Self(owned))
    }

    /// Own one object/alias identifier using the parser's literal-name profile.
    /// Unlike prompt contents, object names cannot contain path punctuation.
    pub fn from_identifier(value: &str, limits: ParseLimits) -> Result<Self, ParseError> {
        // Apply the input bound before scanning punctuation as well as copying.
        if value.len() > limits.max_bytes.min(ParseLimits::MAX_BYTES) || limits.max_tokens == 0 {
            return Self::from_contents(value, limits);
        }
        if value.is_empty()
            || value
                .chars()
                .any(|ch| ch.is_control() || "[]`.!".contains(ch))
        {
            return Err(ParseError {
                kind: ParseErrorKind::Unsupported,
                message: "expected an unreserved or bracketed name",
                location: SourceLocation { line: 1, column: 1 },
            });
        }
        Self::from_contents(value, limits)
    }

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
