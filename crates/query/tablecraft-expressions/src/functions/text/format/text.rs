//! Borrowed text masks with metered two-pass casing and strict UTF-16 publication.
mod render;

use super::lexer::{Kind, Tokens};
use crate::{
    EvalError,
    functions::text::{
        casing::{CaseLocale, Casing},
        output::TextOutput,
    },
};

pub(super) struct Pattern<'a> {
    mask: &'a str,
    slots: usize,
    left: bool,
    uppercase: Option<bool>,
}

pub(super) struct Display<'a> {
    source: &'a str,
    pattern: Pattern<'a>,
}

impl<'a> Pattern<'a> {
    pub fn recognizes(
        mask: &str,
        step: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<bool, EvalError> {
        let mut tokens = Tokens::new(mask);
        while let Some(token) = tokens.next(step)? {
            if matches!(token.kind, Kind::Symbol('@' | '&' | '<' | '>' | '!')) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn parse(mask: &'a str, output: &mut impl TextOutput) -> Result<Self, EvalError> {
        let mut pattern = Self {
            mask,
            slots: 0,
            left: false,
            uppercase: None,
        };
        let mut tokens = Tokens::new(mask);
        while let Some(token) = tokens.next(&mut || output.step())? {
            match token.kind {
                Kind::Symbol('@' | '&') => pattern.slots += 1,
                Kind::Symbol('!') => pattern.left = true,
                Kind::Symbol('>' | '<') => {
                    let uppercase = token.kind == Kind::Symbol('>');
                    if pattern
                        .uppercase
                        .is_some_and(|previous| previous != uppercase)
                    {
                        return Err(EvalError::UnsupportedExpression);
                    }
                    pattern.uppercase = Some(uppercase);
                }
                _ => {}
            }
        }
        Ok(pattern)
    }
}

impl<'a> Display<'a> {
    pub fn new(
        source: &'a str,
        mask: &'a str,
        output: &mut impl TextOutput,
    ) -> Result<Self, EvalError> {
        Ok(Self {
            source,
            pattern: Pattern::parse(mask, output)?,
        })
    }

    pub fn write(
        &self,
        locale: Option<&CaseLocale>,
        output: &mut impl TextOutput,
    ) -> Result<(), EvalError> {
        let mut count = Counter { units: 0, output };
        self.source(locale, &mut count)?;
        let padding = if self.pattern.left {
            0
        } else {
            self.pattern.slots.saturating_sub(count.units)
        };
        let mut renderer = render::Renderer::new(self.pattern.mask, padding, output);
        self.source(locale, &mut renderer)?;
        renderer.finish()
    }

    fn source(
        &self,
        locale: Option<&CaseLocale>,
        output: &mut impl TextOutput,
    ) -> Result<(), EvalError> {
        match self.pattern.uppercase {
            Some(uppercase) if !self.source.is_empty() => {
                Casing::new(self.source, uppercase).write(locale, output)
            }
            _ => output.append(self.source),
        }
    }
}

struct Counter<'a, O> {
    units: usize,
    output: &'a mut O,
}

impl<O: TextOutput> TextOutput for Counter<'_, O> {
    fn step(&mut self) -> Result<(), EvalError> {
        self.output.step()
    }

    fn append(&mut self, text: &str) -> Result<(), EvalError> {
        for _ in text.encode_utf16() {
            self.step()?;
            self.units = self.units.checked_add(1).ok_or(EvalError::TextLimit)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
