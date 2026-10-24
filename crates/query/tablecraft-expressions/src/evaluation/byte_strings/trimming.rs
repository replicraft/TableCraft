//! Scalar trimming keeps its borrowed Text policy; raw Strings retain a view.
use super::{ByteOperand, EvalError, Evaluator, TextFunction};
use crate::functions::text::byte_string::trim_spaces;

impl Evaluator<'_> {
    pub(super) fn apply_trim_source(
        &mut self,
        function: TextFunction,
        first: ByteOperand,
    ) -> Result<ByteOperand, EvalError> {
        match self.coerce_byte_string_source(first)? {
            ByteOperand::Scalar(value) => self
                .apply_text_function(function, std::slice::from_ref(&value))
                .map(ByteOperand::Scalar),
            ByteOperand::Bytes(bytes) => {
                trim_spaces(function, bytes, &mut || self.check(0)).map(ByteOperand::Bytes)
            }
        }
    }
}

#[cfg(test)]
mod tests;
