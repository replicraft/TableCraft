//! Owning byte-string evaluation and strict scalar publication.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        Builtin,
        text::{
            TextFunction,
            byte_string::{ByteOperand, ByteString},
            sql_arguments::{self, Argument},
        },
    },
    numeric::{IntegerType, coerce_integer},
};
use tablecraft_core::{limits::Resource, values::Value};
use tablecraft_sql::select::{Expression, FunctionArgument};

impl Evaluator<'_> {
    pub(super) fn byte_operand(
        &mut self,
        expression: &Expression,
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        if let Expression::Call {
            qualifier,
            name,
            arguments,
        } = expression
            && qualifier.is_empty()
        {
            match Builtin::resolve(name.as_str()) {
                Some(Builtin::Text(function)) if function.produces_bytes() => {
                    self.check(depth)?;
                    return self.byte_call(function, arguments, depth);
                }
                Some(Builtin::CVar) => {
                    if let [FunctionArgument::Expression(value)] = arguments.as_slice() {
                        self.check(depth)?;
                        return self.byte_operand(value, depth + 1);
                    }
                }
                _ => {}
            }
        }
        self.node(expression, depth).map(ByteOperand::Scalar)
    }

    pub(super) fn byte_call(
        &mut self,
        function: TextFunction,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        let mut arguments = sql_arguments::normalize(function, arguments)
            .ok_or(EvalError::UnsupportedExpression)?;
        let Some(Argument::Expression(first)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };
        let first = self.byte_operand(first, depth + 1)?;
        let mut rest = [Value::Null, Value::Null];
        let count = arguments.len();
        for (argument, value) in arguments.zip(&mut rest) {
            *value = match argument {
                Argument::Expression(expression) => self.node(expression, depth + 1)?,
                Argument::Integer(value) => Value::Int64(value),
            };
        }
        self.apply_byte_string(function, first, &rest[..count])
    }

    /// Apply a byte producer to already evaluated operands in written order.
    /// Retains raw string bytes without converting them to stored Binary.
    pub fn apply_byte_string(
        &mut self,
        function: TextFunction,
        first: ByteOperand,
        rest: &[Value],
    ) -> Result<ByteOperand, EvalError> {
        self.check(0)?;
        if !function.produces_bytes() || !function.accepts_arity(rest.len() + 1) {
            return Err(EvalError::UnsupportedExpression);
        }
        if function == TextFunction::ChrB {
            let ByteOperand::Scalar(value) = first else {
                return Err(EvalError::TypeMismatch);
            };
            let byte = u8::try_from(long(&value)?).map_err(|_| EvalError::Overflow)?;
            self.budget.charge(Resource::Bytes, 1)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(1)
                .map_err(|_| EvalError::Allocation)?;
            bytes.push(byte);
            return ByteString::from_owned(bytes).map(ByteOperand::Bytes);
        }

        let position = long(&rest[0])?;
        let length = rest.get(1).map(long).transpose()?;
        if position < 0
            || (function == TextFunction::MidB && position == 0)
            || length.is_some_and(|length| length < 0)
        {
            return Err(EvalError::Domain);
        }
        let bytes = match first {
            ByteOperand::Scalar(Value::Null) => return Ok(ByteOperand::Scalar(Value::Null)),
            ByteOperand::Scalar(Value::Text(text)) => self.encode_byte_string(&text)?,
            ByteOperand::Scalar(_) => return Err(EvalError::TypeMismatch),
            ByteOperand::Bytes(bytes) => bytes,
        };
        let bytes = match function {
            TextFunction::LeftB => bytes.left(position)?,
            TextFunction::RightB => bytes.right(position)?,
            TextFunction::MidB => bytes.mid(position, length)?,
            _ => return Err(EvalError::UnsupportedExpression),
        };
        Ok(ByteOperand::Bytes(bytes))
    }

    pub(super) fn encode_byte_string(&mut self, text: &str) -> Result<ByteString, EvalError> {
        if text.len() > self.limits.max_text_bytes {
            return Err(EvalError::TextLimit);
        }
        let mut length = 0usize;
        for character in text.chars() {
            self.check(0)?;
            length = length
                .checked_add(character.len_utf16() * size_of::<u16>())
                .ok_or(EvalError::Overflow)?;
        }
        i32::try_from(length).map_err(|_| EvalError::Overflow)?;
        self.budget.charge(Resource::Bytes, length as u64)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| EvalError::Allocation)?;
        for unit in text.encode_utf16() {
            self.check(0)?;
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        ByteString::from_owned(bytes)
    }

    /// Publish only ordinary scalars or strictly representable Unicode strings.
    /// Invalid UTF-16 is never replaced, reinterpreted as Binary or truncated.
    pub fn publish_byte_operand(&mut self, operand: ByteOperand) -> Result<Value, EvalError> {
        match operand {
            ByteOperand::Scalar(value) => Ok(value),
            ByteOperand::Bytes(bytes) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                bytes.write_utf8(&mut output)?;
                Ok(Value::Text(output.text))
            }
        }
    }

    /// Observe an intermediate without requiring it to be valid Unicode.
    pub fn observe_byte_operand(
        &mut self,
        function: TextFunction,
        operand: ByteOperand,
    ) -> Result<Value, EvalError> {
        match operand {
            ByteOperand::Scalar(value) => self.apply_text_function(function, &[value]),
            ByteOperand::Bytes(bytes) => {
                self.check(0)?;
                match function {
                    TextFunction::LenB => Ok(Value::Int32(bytes.len())),
                    TextFunction::AscB => bytes
                        .as_bytes()
                        .first()
                        .copied()
                        .map(|byte| Value::Int16(i16::from(byte)))
                        .ok_or(EvalError::Domain),
                    _ => Err(EvalError::UnsupportedExpression),
                }
            }
        }
    }
}

pub(super) fn long(value: &Value) -> Result<i32, EvalError> {
    if matches!(value, Value::Null) {
        return Err(EvalError::InvalidNullConversion);
    }
    match coerce_integer(value, IntegerType::Int32)? {
        Value::Int32(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}

#[cfg(test)]
mod tests;
