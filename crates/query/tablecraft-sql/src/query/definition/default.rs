//! DEFAULT annotations retain syntax without selecting a provider or evaluating it.
use crate::{
    ParseError, ParseErrorKind, ParseLimits, SourceLocation, parse_expression_strict,
    select::{Expression, Name},
};
use std::fmt::{self, Display, Formatter};

/// ACE-engine DEFAULT syntax. Microsoft documents this clause for the Access
/// OLE DB provider/ADO; parsing it does not make it available in SQL View.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColumnDefault {
    /// A single unquoted word, retained without converting it to quoted Text or
    /// a bracketed field reference. Its provider interpretation is not resolved.
    BareWord(BareDefaultWord),
    /// Other written expression syntax, without binding, coercion or evaluation.
    Expression(Box<Expression>),
}

impl Display for ColumnDefault {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::BareWord(word) => formatter.write_str(word.as_str()),
            Self::Expression(expression) => expression.fmt(formatter),
        }
    }
}

/// One validated unquoted default word. It cannot contain SQL punctuation,
/// comments, delimiters, operators, qualifiers or a second token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareDefaultWord(Name);

impl BareDefaultWord {
    /// Own exactly one unquoted word using the parser's actual lexical/name rules.
    /// Reserved literal words such as NULL/TRUE/FALSE belong to expression syntax.
    ///
    /// # Errors
    /// Refuses any other expression or token sequence, and preserves the caller's
    /// byte/token/depth limits and strict parser diagnostics.
    pub fn new(value: &str, limits: ParseLimits) -> Result<Self, ParseError> {
        match parse_expression_strict(value, limits)? {
            Expression::Field(field)
                if field.qualifier.is_none() && field.name.as_str() == value =>
            {
                Ok(Self(field.name))
            }
            _ => Err(ParseError {
                kind: ParseErrorKind::Syntax,
                message: "default word requires one unquoted identifier token",
                location: SourceLocation { line: 1, column: 1 },
            }),
        }
    }

    // The declaration parser has already proved one unquoted token and an
    // unqualified field expression. Moving that owner avoids parsing/copying it twice.
    pub(crate) fn from_token(name: Name) -> Self {
        Self(name)
    }

    /// Exact unquoted token spelling, without provider interpretation.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
