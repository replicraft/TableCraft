//! Streaming placement; an interrupted surrogate pair is never replaced or lost.
use super::{Kind, Tokens};
use crate::{EvalError, functions::text::output::TextOutput};

pub(super) struct Renderer<'a, 'o, O> {
    tokens: Tokens<'a>,
    padding: usize,
    pending: Option<u16>,
    output: &'o mut O,
}

impl<'a, 'o, O: TextOutput> Renderer<'a, 'o, O> {
    pub fn new(mask: &'a str, padding: usize, output: &'o mut O) -> Self {
        Self {
            tokens: Tokens::new(mask),
            padding,
            pending: None,
            output,
        }
    }

    fn literal(&mut self, text: &str) -> Result<(), EvalError> {
        if !text.is_empty() && self.pending.is_some() {
            return Err(EvalError::UnrepresentableText);
        }
        self.output.append(text)
    }

    fn unit(&mut self, unit: u16) -> Result<(), EvalError> {
        self.output.step()?;
        let character = if let Some(first) = self.pending.take() {
            char::decode_utf16([first, unit])
                .next()
                .ok_or(EvalError::UnrepresentableText)?
                .map_err(|_| EvalError::UnrepresentableText)?
        } else {
            match char::decode_utf16([unit]).next() {
                Some(Ok(character)) => character,
                _ => {
                    self.pending = Some(unit);
                    return Ok(());
                }
            }
        };
        let mut buffer = [0; char::MAX.len_utf8()];
        self.output.append(character.encode_utf8(&mut buffer))
    }

    fn slot(&mut self) -> Result<Option<char>, EvalError> {
        while let Some(token) = self.tokens.next(&mut || self.output.step())? {
            match token.kind {
                Kind::Symbol(symbol @ ('@' | '&')) => {
                    if self.padding == 0 {
                        return Ok(Some(symbol));
                    }
                    self.padding -= 1;
                    if symbol == '@' {
                        self.literal(" ")?;
                    }
                }
                Kind::Symbol('!' | '<' | '>') => {}
                Kind::Literal(text) => self.literal(text)?,
                Kind::Symbol(symbol) => {
                    let mut buffer = [0; char::MAX.len_utf8()];
                    self.literal(symbol.encode_utf8(&mut buffer))?;
                }
            }
        }
        Ok(None)
    }

    pub fn finish(mut self) -> Result<(), EvalError> {
        while let Some(symbol) = self.slot()? {
            if symbol == '@' {
                self.literal(" ")?;
            }
        }
        if self.pending.is_some() {
            return Err(EvalError::UnrepresentableText);
        }
        self.output.step()
    }
}

impl<O: TextOutput> TextOutput for Renderer<'_, '_, O> {
    fn step(&mut self) -> Result<(), EvalError> {
        self.output.step()
    }

    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        for unit in text.encode_utf16() {
            self.slot()?;
            self.unit(unit)?;
        }
        Ok(())
    }
}
