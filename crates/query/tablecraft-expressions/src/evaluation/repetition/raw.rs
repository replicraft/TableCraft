//! String retains its first complete unit until an explicit publication boundary.
use super::{Evaluator, construction, text_arguments};
use crate::{
    EvalError,
    evaluation::byte_output,
    functions::{
        conversions::Conversion,
        text::{
            TextFunction,
            byte_string::{ByteOperand, ByteString},
            output::ByteOutput,
        },
    },
};
use tablecraft_core::values::{ExpressionString, ExpressionValue, Value};
use tablecraft_sql::select::FunctionArgument;

const UNIT_BYTES: usize = size_of::<u16>();

#[derive(Clone, Copy)]
enum Input<'a> {
    Expression(&'a ExpressionValue),
    Value(&'a Value),
}

impl Input<'_> {
    fn is_null(&self) -> bool {
        matches!(
            self,
            Self::Expression(ExpressionValue::Value(Value::Null)) | Self::Value(Value::Null)
        )
    }

    fn scalar<'a>(&'a self, empty: &'a Value) -> Result<&'a Value, EvalError> {
        match self {
            Self::Expression(value) => text_arguments::scalar(value, empty),
            Self::Value(value) => Ok(value),
        }
    }

    fn raw(&self) -> Option<&ExpressionString> {
        match self {
            Self::Expression(ExpressionValue::StringBytes(bytes)) => Some(bytes),
            _ => None,
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Expression(ExpressionValue::Value(Value::Text(text)))
            | Self::Value(Value::Text(text)) => Some(text),
            _ => None,
        }
    }
}

impl Evaluator<'_> {
    pub(in crate::evaluation) fn string_byte_call(
        &mut self,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        let [
            FunctionArgument::Expression(count),
            FunctionArgument::Expression(character),
        ] = arguments
        else {
            return Err(EvalError::UnsupportedExpression);
        };

        let count = self
            .expression_byte_operand(count, depth + 1)?
            .into_expression_value();
        let character = self
            .expression_byte_operand(character, depth + 1)?
            .into_expression_value();
        self.apply_retained_string(&count, &character)
    }

    /// Repeat a literal first String unit, preserving raw results. Numeric
    /// characters retain checked Long/modulo/code-page conversion. Written
    /// operands must already be evaluated eagerly; NULL precedes conversion.
    pub fn apply_retained_string(
        &mut self,
        count: &ExpressionValue,
        character: &ExpressionValue,
    ) -> Result<ByteOperand, EvalError> {
        self.string_inputs(Input::Expression(count), Input::Expression(character))
    }

    pub(in crate::evaluation) fn repeat_scalar_character(
        &mut self,
        count: &ExpressionValue,
        character: &Value,
    ) -> Result<ByteOperand, EvalError> {
        self.string_inputs(Input::Expression(count), Input::Value(character))
    }

    fn string_inputs(
        &mut self,
        count: Input<'_>,
        character: Input<'_>,
    ) -> Result<ByteOperand, EvalError> {
        for input in [count, character] {
            if input
                .text()
                .is_some_and(|text| text.len() > self.limits.max_text_bytes)
            {
                return Err(EvalError::TextLimit);
            }
        }

        self.check(0)?;
        if count.is_null() || character.is_null() {
            return Ok(ByteOperand::Scalar(Value::Null));
        }

        let empty_number = Value::Int32(0);
        let published_count = count
            .raw()
            .map(|bytes| self.publish_expression_string(bytes))
            .transpose()?;
        let count = match published_count.as_ref() {
            Some(value) => value,
            None => count.scalar(&empty_number)?,
        };
        let Value::Int32(count) = self.apply_conversion(Conversion::Long, count)? else {
            return Err(EvalError::TypeMismatch);
        };
        if count < 0 {
            return Err(EvalError::Domain);
        }

        let unit = if let Some(bytes) = character.raw() {
            i32::try_from(bytes.len()).map_err(|_| EvalError::Overflow)?;
            let (units, remainder) = bytes.as_bytes().as_chunks::<UNIT_BYTES>();
            if !remainder.is_empty() {
                return Err(EvalError::UnrepresentableText);
            }
            u16::from_le_bytes(*units.first().ok_or(EvalError::Domain)?)
        } else if let Some(text) = character.text() {
            text.encode_utf16().next().ok_or(EvalError::Domain)?
        } else {
            let character = character.scalar(&empty_number)?;
            let result = construction::repeat(
                TextFunction::String,
                count,
                Some(character),
                self.character_code_page,
            )?;
            return self
                .materialize_text_result(result)
                .map(ByteOperand::Scalar);
        };

        // A valid scalar unit retains the existing streamed UTF-8 path. Raw
        // source tails are not decoded: only their first complete unit is used.
        if let Some(character) = char::from_u32(u32::from(unit)) {
            let result = construction::repeat_character(character, count)?;
            return self
                .materialize_text_result(result)
                .map(ByteOperand::Scalar);
        }

        let mut output = byte_output::Output {
            budget: self.budget,
            cancelled: self.cancelled,
            max_bytes: i32::MAX as usize,
            bytes: Vec::new(),
        };
        for _ in 0..count {
            output.append(&unit.to_le_bytes())?;
        }
        ByteString::from_owned(output.bytes).map(ByteOperand::Bytes)
    }
}

#[cfg(test)]
mod tests;
