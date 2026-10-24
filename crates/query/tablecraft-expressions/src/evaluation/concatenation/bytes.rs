//! Concatenation retains complete UTF-16 units until an explicit publication.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::errors::into_operator_scalar,
        text::byte_string::{ByteOperand, ByteString, ExpressionByteOperand},
    },
};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};
use tablecraft_sql::select::Expression;

const UNIT_BYTES: i32 = size_of::<u16>() as i32;
const COPY_CHUNK_BYTES: usize = 4096;

impl Evaluator<'_> {
    pub(in crate::evaluation) fn concatenation_call(
        &mut self,
        left: &Expression,
        right: &Expression,
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        let left = self.expression_byte_operand(left, depth + 1)?;
        let right = self.expression_byte_operand(right, depth + 1)?;
        self.apply_expression_concatenation(left, right)
    }

    /// Join already evaluated operands, retaining raw string units. Ordinary
    /// scalar pairs keep the existing conversion and borrowed-source path.
    pub fn apply_expression_concatenation(
        &mut self,
        left: ExpressionByteOperand,
        right: ExpressionByteOperand,
    ) -> Result<ByteOperand, EvalError> {
        let left = admit(left)?;
        let right = admit(right)?;
        if let (ByteOperand::Scalar(left), ByteOperand::Scalar(right)) = (&left, &right) {
            return self.concatenate(left, right).map(ByteOperand::Scalar);
        }

        self.check(0)?;
        let left = self.concatenation_bytes(left)?;
        let right = self.concatenation_bytes(right)?;
        let (left, right) = match (left, right) {
            (Some(bytes), None) | (None, Some(bytes)) => return Ok(ByteOperand::Bytes(bytes)),
            (Some(left), Some(right)) => (left, right),
            (None, None) => return Ok(ByteOperand::Scalar(Value::Null)),
        };

        let length = left
            .len()
            .checked_add(right.len())
            .ok_or(EvalError::Overflow)?;
        self.budget.charge(Resource::Bytes, length as u64)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length as usize)
            .map_err(|_| EvalError::Allocation)?;
        for source in [left.as_bytes(), right.as_bytes()] {
            for chunk in source.chunks(COPY_CHUNK_BYTES) {
                self.check(0)?;
                bytes.extend_from_slice(chunk);
            }
        }
        ByteString::from_owned(bytes).map(ByteOperand::Bytes)
    }

    fn concatenation_bytes(&mut self, value: ByteOperand) -> Result<Option<ByteString>, EvalError> {
        let value = self.coerce_byte_string_source(value)?;
        match value {
            ByteOperand::Scalar(Value::Null) => Ok(None),
            ByteOperand::Scalar(Value::Text(text)) => self.encode_byte_string(&text).map(Some),
            ByteOperand::Bytes(bytes) => {
                if bytes.len() % UNIT_BYTES != 0 {
                    return Err(EvalError::UnrepresentableText);
                }
                Ok(Some(bytes))
            }
            _ => Err(EvalError::TypeMismatch),
        }
    }
}

fn admit(value: ExpressionByteOperand) -> Result<ByteOperand, EvalError> {
    match value {
        ExpressionByteOperand::Scalar(ExpressionValue::Empty) => {
            Ok(ByteOperand::Scalar(Value::Text(String::new())))
        }
        ExpressionByteOperand::Scalar(value) => {
            into_operator_scalar(value).map(ByteOperand::Scalar)
        }
        ExpressionByteOperand::Bytes(bytes) => Ok(ByteOperand::Bytes(bytes)),
    }
}

#[cfg(test)]
mod tests;
