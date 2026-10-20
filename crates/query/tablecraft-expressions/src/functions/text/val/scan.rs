//! Whitespace-aware source scanning; cloned lookahead remains charged work.
use super::NumericBuffer;
use crate::EvalError;
#[derive(Clone)]
pub(super) struct Cursor<'a> {
    characters: std::str::Chars<'a>,
}
impl<'a> Cursor<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            characters: text.chars(),
        }
    }
    pub fn next(&mut self, buffer: &mut impl NumericBuffer) -> Result<Option<char>, EvalError> {
        for ch in self.characters.by_ref() {
            buffer.step()?;
            if !matches!(ch, ' ' | '\t' | '\n') {
                return Ok(Some(ch));
            }
        }
        Ok(None)
    }
}
