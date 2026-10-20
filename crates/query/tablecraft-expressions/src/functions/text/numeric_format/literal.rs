//! Stack-only formatting storage; never formats database/application display cells.
use crate::EvalError;
use std::fmt;

use tablecraft_core::values::Decimal;

// A scale-28/29-digit fixed value needs sign, leading zero and point at most.
// This also covers a 64-bit octal spelling and a 15-digit scientific literal;
// expanded floating fractions stream directly to the charged host, not here.
const LITERAL_BYTES: usize = Decimal::MAX_PRECISION as usize + 3;

pub(super) struct Literal {
    bytes: [u8; LITERAL_BYTES],
    length: usize,
}
impl Literal {
    pub fn new() -> Self {
        Self {
            bytes: [0; LITERAL_BYTES],
            length: 0,
        }
    }
    pub fn text(&self) -> Result<&str, EvalError> {
        std::str::from_utf8(&self.bytes[..self.length]).map_err(|_| EvalError::UnrepresentableText)
    }
}
impl fmt::Write for Literal {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
