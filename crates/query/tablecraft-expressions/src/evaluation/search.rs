//! Contextual substring operands retain explicit formatting/search policies.
mod raw;

use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, search},
    },
};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};

impl Evaluator<'_> {
    pub(super) fn contextual_search(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<TextResult<'static>, EvalError> {
        let (haystack, needle) = search::operand_indices(function, arguments.len());
        let null_operand =
            matches!(arguments[haystack], Value::Null) || matches!(arguments[needle], Value::Null);

        self.search_inputs(function, arguments.len(), null_operand, |index| {
            Ok(&arguments[index])
        })
    }

    /// Admit already evaluated mode/start controls before string-source states.
    pub fn apply_expression_search(
        &mut self,
        function: TextFunction,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !matches!(function, TextFunction::InStr | TextFunction::InStrRev)
            || !function.accepts_arity(arguments.len())
        {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(arguments)?;

        let (haystack, needle) = search::operand_indices(function, arguments.len());
        let null_operand = matches!(arguments[haystack], ExpressionValue::Value(Value::Null))
            || matches!(arguments[needle], ExpressionValue::Value(Value::Null));
        if matches!(arguments[haystack], ExpressionValue::StringBytes(_))
            || matches!(arguments[needle], ExpressionValue::StringBytes(_))
        {
            return self.raw_search(function, arguments, null_operand);
        }
        let empty_text = Value::Text(String::new());
        let empty_number = Value::Int32(0);
        let result = self.search_inputs(function, arguments.len(), null_operand, |index| {
            let empty = if index == haystack || index == needle {
                &empty_text
            } else {
                &empty_number
            };
            text_arguments::scalar(&arguments[index], empty)
        })?;

        match result {
            TextResult::Null => Ok(Value::Null),
            TextResult::Position(value) => Ok(Value::Int32(value)),
            _ => Err(EvalError::UnsupportedExpression),
        }
    }

    fn search_inputs<'a>(
        &mut self,
        function: TextFunction,
        count: usize,
        null_operand: bool,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<TextResult<'static>, EvalError> {
        let (options, start) = self.search_controls(function, count, &mut argument)?;

        // NULL is observed without coercing its partner, after valid controls.
        // Every written SQL operand has already been evaluated at the caller.
        if null_operand {
            return Ok(TextResult::Null);
        }

        let (haystack, needle) = search::operand_indices(function, count);
        let haystack = argument(haystack)?;
        let converted_haystack = self.string_operand(haystack)?;
        let needle = argument(needle)?;
        let converted_needle = self.string_operand(needle)?;

        options.find_operands(
            start,
            converted_haystack.as_ref().unwrap_or(haystack),
            converted_needle.as_ref().unwrap_or(needle),
            self.collation,
            &mut || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            },
        )
    }

    fn search_controls<'a>(
        &mut self,
        function: TextFunction,
        count: usize,
        mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    ) -> Result<(search::SearchOptions, i32), EvalError> {
        self.check(0)?;
        let mode = if count > 3 { Some(argument(3)?) } else { None };
        let normalized_mode = self.text_long_override(mode)?;
        let options = search::prepare_options(function, count, normalized_mode.as_ref().or(mode))?;

        let start = match search::start_index(function, count) {
            Some(index) => match self.apply_conversion(Conversion::Long, argument(index)?)? {
                Value::Int32(value) => value,
                _ => return Err(EvalError::TypeMismatch),
            },
            None => options.default_start(),
        };
        options.validate_start(start)?;

        Ok((options, start))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scalar_tests;

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod raw_tests;

#[cfg(test)]
mod raw_provider_tests;

#[cfg(test)]
mod reverse_bounds_tests;
