//! Borrowed format-mask tokens preserve quoted/escaped characters and byte spans.
use crate::EvalError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind<'a> {
    Literal(&'a str),
    Symbol(char),
}

pub(super) struct Token<'a> {
    pub kind: Kind<'a>,
    pub start: usize,
    pub end: usize,
}

pub(super) struct Tokens<'a> {
    source: &'a str,
    cursor: usize,
}

impl<'a> Tokens<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source, cursor: 0 }
    }

    pub fn remaining(&self) -> &'a str {
        &self.source[self.cursor..]
    }

    pub fn consume_ascii(
        &mut self,
        bytes: usize,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<(), EvalError> {
        let fragment = self.remaining().get(..bytes).ok_or(EvalError::Domain)?;
        if !fragment.is_ascii() {
            return Err(EvalError::Domain);
        }
        for _ in fragment.bytes() {
            step()?;
        }
        self.cursor += bytes;
        Ok(())
    }

    pub fn next(
        &mut self,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Option<Token<'a>>, EvalError> {
        let start = self.cursor;
        let Some(character) = self.source[start..].chars().next() else {
            return Ok(None);
        };
        step()?;
        self.cursor += character.len_utf8();
        let kind = match character {
            '\\' => {
                let literal = self.cursor;
                let character = self.source[literal..]
                    .chars()
                    .next()
                    .ok_or(EvalError::Domain)?;
                step()?;
                self.cursor += character.len_utf8();
                Kind::Literal(&self.source[literal..self.cursor])
            }
            '"' => {
                let literal = self.cursor;
                loop {
                    let position = self.cursor;
                    let character = self.source[position..]
                        .chars()
                        .next()
                        .ok_or(EvalError::Domain)?;
                    step()?;
                    self.cursor += character.len_utf8();
                    if character == '"' {
                        break Kind::Literal(&self.source[literal..position]);
                    }
                }
            }
            symbol => Kind::Symbol(symbol),
        };
        Ok(Some(Token {
            kind,
            start,
            end: self.cursor,
        }))
    }
}

pub(super) fn literal(symbol: char) -> bool {
    matches!(symbol, ' ' | '-' | '+' | '$' | '(' | ')')
}
