use sqlparser::tokenizer::{Location, Token, TokenWithSpan};

use super::{ParseError, ParseErrorKind};

/// Caller-lowerable limits. Values above hard ceilings are clamped to them.
/// Tokenization is bounded by bytes before it allocates its token vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    /// Maximum UTF-8 bytes; hard ceiling 65,536.
    pub max_bytes: usize,
    /// Maximum non-whitespace tokens; hard ceiling 1,024.
    pub max_tokens: usize,
    /// Maximum parenthesis nesting and owned expression depth; hard ceiling 32.
    pub max_depth: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_bytes: 65_536,
            max_tokens: 1_024,
            max_depth: 32,
        }
    }
}

impl ParseLimits {
    pub(super) fn check_input(self, source: &str) -> Result<(), ParseError> {
        if source.len() > self.max_bytes.min(65_536) {
            return Err(ParseError::new(
                ParseErrorKind::Limit,
                "SQL byte limit exceeded",
                Location { line: 1, column: 1 },
            ));
        }
        Ok(())
    }

    pub(super) fn depth(self) -> usize {
        self.max_depth.min(32)
    }

    pub(super) fn check_tokens(
        self,
        source: &str,
        tokens: Vec<TokenWithSpan>,
    ) -> Result<Vec<TokenWithSpan>, ParseError> {
        let mut output = Vec::new();
        let mut depth = 0_usize;
        let lines: Vec<_> = source.split('\n').collect();
        for token in tokens {
            if matches!(token.token, Token::Whitespace(_)) {
                continue;
            }
            if output.len() >= self.max_tokens.min(1_024) {
                return Err(ParseError::new(
                    ParseErrorKind::Limit,
                    "SQL token limit exceeded",
                    token.span.start,
                ));
            }
            match token.token {
                // The lexer collapses both <> and != into Neq. Inspect original
                // spelling before accepting the Access comparison operator.
                Token::Neq
                    if lines
                        .get(token.span.start.line as usize - 1)
                        .and_then(|line| {
                            line.chars().nth(token.span.start.column as usize - 1)
                        })
                        == Some('!') =>
                {
                    return Err(ParseError::new(
                        ParseErrorKind::Unsupported,
                        "use the Access <> comparison operator",
                        token.span.start,
                    ));
                }
                Token::DoubleEq => {
                    return Err(ParseError::new(
                        ParseErrorKind::Unsupported,
                        "use the Access = comparison operator",
                        token.span.start,
                    ));
                }
                Token::LParen => {
                    depth += 1;
                    if depth > self.depth() {
                        return Err(ParseError::new(
                            ParseErrorKind::Limit,
                            "SQL nesting limit exceeded",
                            token.span.start,
                        ));
                    }
                }
                Token::RParen => {
                    depth = depth.checked_sub(1).ok_or_else(|| {
                        ParseError::new(
                            ParseErrorKind::Syntax,
                            "unmatched closing parenthesis",
                            token.span.start,
                        )
                    })?;
                }
                _ => {}
            }
            output.push(token);
        }
        if depth != 0 {
            let location = output
                .last()
                .map_or(Location { line: 1, column: 1 }, |t| t.span.end);
            return Err(ParseError::new(
                ParseErrorKind::Syntax,
                "unclosed parenthesis",
                location,
            ));
        }
        Ok(output)
    }
}

/// Shared across nested expressions, relations and subqueries within one parse.
pub(super) struct ParseContext {
    limits: ParseLimits,
    recursion: std::cell::Cell<usize>,
}
impl std::ops::Deref for ParseContext {
    type Target = ParseLimits;
    fn deref(&self) -> &ParseLimits {
        &self.limits
    }
}
impl ParseContext {
    pub(super) fn new(limits: ParseLimits) -> Self {
        Self {
            limits,
            recursion: std::cell::Cell::new(0),
        }
    }
    pub(super) fn enter(
        &self,
        parser: &sqlparser::parser::Parser<'_>,
    ) -> Result<RecursionGuard<'_>, ParseError> {
        if self.recursion.get() >= 96 {
            return Err(ParseError::at(
                parser,
                ParseErrorKind::Limit,
                "SQL parser recursion limit exceeded",
            ));
        }
        self.recursion.set(self.recursion.get() + 1);
        Ok(RecursionGuard(&self.recursion))
    }
}
pub(super) struct RecursionGuard<'a>(&'a std::cell::Cell<usize>);
impl Drop for RecursionGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}
