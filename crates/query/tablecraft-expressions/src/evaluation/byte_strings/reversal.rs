//! Reversed String units remain retained until an explicit scalar boundary.
use super::{ByteOperand, ByteString, EvalError, Evaluator};
use crate::{evaluation::byte_output, functions::text::byte_string::write_reversed_units};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn apply_reverse_source(
        &mut self,
        source: ByteOperand,
    ) -> Result<ByteOperand, EvalError> {
        let source = match self.coerce_byte_string_source(source)? {
            ByteOperand::Scalar(Value::Null) => return Err(EvalError::InvalidNullConversion),
            ByteOperand::Scalar(Value::Text(text)) => self.encode_byte_string(&text)?,
            ByteOperand::Bytes(bytes) => bytes,
            _ => return Err(EvalError::TypeMismatch),
        };
        let mut output = byte_output::Output {
            budget: self.budget,
            cancelled: self.cancelled,
            max_bytes: i32::MAX as usize,
            bytes: Vec::new(),
        };
        write_reversed_units(&source, &mut output)?;
        ByteString::from_owned(output.bytes).map(ByteOperand::Bytes)
    }
}

#[cfg(test)]
mod tests;
