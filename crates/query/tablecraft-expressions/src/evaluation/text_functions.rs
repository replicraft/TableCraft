//! Budgeted materialization of borrowed text-function results.
use super::Evaluator;
use crate::{
    EvalError,
    functions::text::{self, TextFunction, TextResult},
};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    /// Apply a closed text built-in using this evaluator's shared scan/allocation
    /// limits. Operands must already be evaluated in written order.
    pub fn apply_text_function(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<Value, EvalError> {
        for value in arguments {
            if matches!(value, Value::Text(text) if text.len() > self.limits.max_text_bytes) {
                return Err(EvalError::TextLimit);
            }
        }
        let result = text::apply_with_context(
            function,
            arguments,
            text::TextContext {
                collation: self.collation,
                character_code_page: self.character_code_page,
                number_format: self.number_format,
                date_format: self.date_format,
                year_window: self.year_window,
                reference_year: self.reference_year,
            },
            || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            },
        )?;
        match result {
            TextResult::Null => Ok(Value::Null),
            TextResult::Comparison(value) | TextResult::CharacterCode(value) => {
                Ok(Value::Int16(value))
            }
            TextResult::Character(character) => {
                let mut buffer = [0; char::MAX.len_utf8()];
                self.text(character.encode_utf8(&mut buffer))
            }
            TextResult::Length(length) | TextResult::Position(length) => Ok(Value::Int32(length)),
            TextResult::Text(text) => self.text(text),
            TextResult::DateFormat(format) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                format.write(&mut output)?;
                Ok(Value::Text(output.text))
            }
            TextResult::DateTimeFormat(format) => {
                let display = {
                    let mut scratch = super::text_output::Output {
                        budget: self.budget,
                        cancelled: self.cancelled,
                        max_bytes: self.limits.max_text_bytes,
                        text: String::new(),
                    };
                    format.resolve(&mut scratch)?
                };
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                display.write(&mut output)?;
                Ok(Value::Text(output.text))
            }
            TextResult::NumericFormat(format) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                format.write(&mut output)?;
                Ok(Value::Text(output.text))
            }
            TextResult::NumericPrefix(prefix) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                Ok(Value::Double(prefix.evaluate(&mut output)?))
            }
            TextResult::Construction(construction) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                construction.write(&mut output)?;
                Ok(Value::Text(output.text))
            }
            TextResult::Replacement(replacement) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                replacement.write(self.collation, &mut output)?;
                Ok(Value::Text(output.text))
            }
        }
    }
}

#[cfg(test)]
mod date_format_tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::dates::parsing::DateTextFormat,
    };
    use tablecraft_core::{
        limits::{Resource, ResourceBudget},
        values::LegacyDateTime,
    };
    #[test]
    fn named_date_output_obeys_allocation_work_text_and_cancellation_limits() {
        let arguments = [Value::DateTime(LegacyDateTime::from_ole_days(1.0).unwrap())];
        let mut budget = ResourceBudget::new(3, 0, 100);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_date_format(DateTextFormat::EnUs).apply_text_function(TextFunction::FormatDateTime, &arguments), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
        );
        let mut budget = ResourceBudget::new(100, 0, 1);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_date_format(DateTextFormat::EnUs).apply_text_function(TextFunction::FormatDateTime, &arguments), Err(EvalError::Limit(error)) if error.resource == Resource::Steps)
        );
        let mut budget = ResourceBudget::new(100, 0, 100);
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 3,
                    ..Default::default()
                }
            )
            .with_date_format(DateTextFormat::EnUs)
            .apply_text_function(TextFunction::FormatDateTime, &arguments),
            Err(EvalError::TextLimit)
        );
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 3,
                    ..Default::default()
                },
            )
            .with_date_format(DateTextFormat::Iso)
            .apply_text_function(
                TextFunction::FormatDateTime,
                &[Value::Text("2026-10-20".into())]
            ),
            Err(EvalError::TextLimit)
        );
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_text_function(TextFunction::FormatDateTime, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}
