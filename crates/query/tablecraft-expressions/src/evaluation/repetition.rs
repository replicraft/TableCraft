//! Repetition count normalization retains String's literal character argument.
mod raw;

use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, construction},
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Preserve eager expression states through the NULL gate, count and character
    /// admission. Empty has a numeric target in both slots; Text characters stay literal.
    pub fn apply_expression_repeat(
        &mut self,
        function: TextFunction,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !matches!(function, TextFunction::Space | TextFunction::String)
            || !function.accepts_arity(arguments.len())
        {
            return Err(EvalError::UnsupportedExpression);
        }
        if function == TextFunction::String {
            let result = self.apply_retained_string(&arguments[0], &arguments[1])?;
            return self.publish_byte_operand(result);
        }
        self.text_expression_limits(arguments)?;
        self.check(0)?;
        if arguments
            .iter()
            .any(|value| matches!(value, ExpressionValue::Value(Value::Null)))
        {
            return Ok(Value::Null);
        }

        let empty_number = Value::Int32(0);
        let count = text_arguments::scalar(&arguments[0], &empty_number)?;
        let Value::Int32(count) = self.apply_conversion(Conversion::Long, count)? else {
            return Err(EvalError::TypeMismatch);
        };
        if count < 0 {
            return Err(EvalError::Domain);
        }

        let character = arguments
            .get(1)
            .map(|value| text_arguments::scalar(value, &empty_number))
            .transpose()?;
        let result = construction::repeat(function, count, character, self.character_code_page)?;
        self.materialize_text_result(result)
    }

    pub(super) fn contextual_repeat(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<TextResult<'static>, EvalError> {
        self.check(0)?;
        let Value::Int32(count) = self.apply_conversion(Conversion::Long, &arguments[0])? else {
            return Err(EvalError::TypeMismatch);
        };
        construction::repeat(function, count, arguments.get(1), self.character_code_page)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod expression_tests;
