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
        if function == TextFunction::Str
            && function.accepts_arity(arguments.len())
            && matches!(&arguments[0], Value::DateTime(_))
        {
            return self.apply_text_function(TextFunction::CStr, arguments);
        }
        if matches!(
            function,
            TextFunction::Hex | TextFunction::Oct | TextFunction::Str
        ) && function.accepts_arity(arguments.len())
            && matches!(&arguments[0], Value::Text(_))
        {
            return self.contextual_numeric_text(function, &arguments[0]);
        }
        if matches!(
            function,
            TextFunction::Len
                | TextFunction::LenB
                | TextFunction::LTrim
                | TextFunction::RTrim
                | TextFunction::Trim
                | TextFunction::LCase
                | TextFunction::UCase
                | TextFunction::Asc
                | TextFunction::AscW
                | TextFunction::AscB
                | TextFunction::StrReverse
                | TextFunction::Val
        ) && function.accepts_arity(arguments.len())
            && !matches!(&arguments[0], Value::Text(_) | Value::Null)
        {
            return self.contextual_string_function(function, &arguments[0]);
        }
        let result = if function == TextFunction::Partition
            && function.accepts_arity(arguments.len())
            && arguments
                .iter()
                .any(|value| matches!(value, Value::Text(_)))
        {
            self.contextual_partition(arguments)?
        } else if matches!(
            function,
            TextFunction::Left | TextFunction::Right | TextFunction::Mid
        ) && function.accepts_arity(arguments.len())
        {
            return self.contextual_slice(function, arguments);
        } else if matches!(function, TextFunction::Space | TextFunction::String)
            && function.accepts_arity(arguments.len())
            && matches!(&arguments[0], Value::Text(_))
            && !arguments.iter().any(|value| matches!(value, Value::Null))
        {
            self.contextual_repeat(function, arguments)?
        } else if matches!(function, TextFunction::InStr | TextFunction::InStrRev)
            && function.accepts_arity(arguments.len())
        {
            self.contextual_search(function, arguments)?
        } else if function == TextFunction::Replace && function.accepts_arity(arguments.len()) {
            return self.contextual_replacement(arguments);
        } else if function == TextFunction::StrComp && function.accepts_arity(arguments.len()) {
            self.contextual_comparison(arguments)?
        } else if matches!(
            function,
            TextFunction::Chr | TextFunction::ChrString | TextFunction::ChrW
        ) && function.accepts_arity(arguments.len())
            && matches!(arguments.first(), Some(Value::Text(_)))
        {
            self.contextual_character(function, &arguments[0])?
        } else if matches!(
            function,
            TextFunction::FormatNumber | TextFunction::FormatPercent | TextFunction::FormatCurrency
        ) && function.accepts_arity(arguments.len())
            && arguments
                .iter()
                .skip(1)
                .any(|value| matches!(value, Value::Text(_)))
        {
            self.contextual_numeric_format(function, arguments)?
        } else if function == TextFunction::Format
            && function.accepts_arity(arguments.len())
            && arguments
                .iter()
                .skip(2)
                .any(|value| matches!(value, Value::Text(_)))
        {
            self.contextual_week_format(arguments)?
        } else if function == TextFunction::FormatDateTime
            && function.accepts_arity(arguments.len())
            && matches!(arguments.get(1), Some(Value::Text(_)))
        {
            self.contextual_date_format(arguments)?
        } else {
            text::apply_with_context(
                function,
                arguments,
                text::TextContext {
                    collation: self.collation,
                    character_code_page: self.character_code_page,
                    number_format: self.number_format,
                    date_format: self.date_format,
                    year_window: self.year_window,
                    reference_year: self.reference_year,
                    first_day_of_week: self.first_day_of_week,
                    first_week_of_year: self.first_week_of_year,
                },
                || {
                    if self.cancelled.is_some_and(|cancelled| cancelled()) {
                        return Err(EvalError::Cancelled);
                    }
                    self.budget.charge(Resource::Steps, 1)?;
                    Ok(())
                },
            )?
        };
        self.materialize_text_result(result)
    }

    pub(super) fn materialize_text_result(
        &mut self,
        result: TextResult<'_>,
    ) -> Result<Value, EvalError> {
        match result {
            TextResult::Casing(casing) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                casing.write(self.case_locale, &mut output)?;
                Ok(Value::Text(output.text))
            }
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
            TextResult::Format(format) => {
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
                display.write_with_case_locale(self.case_locale, &mut output)?;
                Ok(Value::Text(output.text))
            }
            TextResult::StyledNumber(format) => {
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
            TextResult::CalendarName(name) => {
                let mut output = super::text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                name.write(&mut output)?;
                Ok(Value::Text(output.text))
            }
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
            TextResult::Partition(format) => {
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

#[cfg(test)]
mod calendar_name_tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::dates::parsing::DateTextFormat,
    };
    use tablecraft_core::limits::{Resource, ResourceBudget};
    #[test]
    fn calendar_names_obey_output_work_and_cancellation_limits() {
        let arguments = [Value::Int16(3)];
        let mut budget = ResourceBudget::new(2, 0, 100);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_date_format(DateTextFormat::DeDe).apply_text_function(TextFunction::MonthName, &arguments), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
        );
        let mut budget = ResourceBudget::new(100, 0, 1);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_date_format(DateTextFormat::DeDe).apply_text_function(TextFunction::MonthName, &arguments), Err(EvalError::Limit(error)) if error.resource == Resource::Steps)
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
            .with_date_format(DateTextFormat::DeDe)
            .apply_text_function(TextFunction::MonthName, &arguments),
            Err(EvalError::TextLimit)
        );
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_text_function(TextFunction::MonthName, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}

#[cfg(test)]
mod styled_number_tests {
    use super::*;
    use crate::{
        evaluation::{EvalLimits, NoBindings},
        functions::conversions::text::NumberFormat,
    };
    use tablecraft_core::limits::{Resource, ResourceBudget};
    #[test]
    fn numeric_formatting_enforces_scratch_output_work_input_and_cancellation_limits() {
        let arguments = [Value::Int32(1234)];
        let mut budget = ResourceBudget::new(3, 0, 100);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_number_format(NumberFormat::EnUs).apply_text_function(TextFunction::FormatNumber, &arguments), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
        );
        let mut budget = ResourceBudget::new(100, 0, 1);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_number_format(NumberFormat::EnUs).apply_text_function(TextFunction::FormatNumber, &arguments), Err(EvalError::Limit(error)) if error.resource == Resource::Steps)
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
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::FormatNumber, &arguments),
            Err(EvalError::TextLimit)
        );
        assert_eq!(
            Evaluator::new(
                &NoBindings,
                &mut budget,
                EvalLimits {
                    max_text_bytes: 3,
                    ..Default::default()
                }
            )
            .with_number_format(NumberFormat::EnUs)
            .apply_text_function(TextFunction::FormatNumber, &[Value::Text("1.25".into())]),
            Err(EvalError::TextLimit)
        );
        let mut budget = ResourceBudget::new(3, 0, 100);
        assert!(
            matches!(Evaluator::new(&NoBindings, &mut budget, EvalLimits::default()).with_number_format(NumberFormat::Invariant).apply_text_function(TextFunction::FormatNumber, &[Value::Text("1.25".into())]), Err(EvalError::Limit(error)) if error.resource == Resource::Bytes)
        );
        assert_eq!(
            Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_cancellation(&|| true)
                .apply_text_function(TextFunction::FormatNumber, &arguments),
            Err(EvalError::Cancelled)
        );
    }
}

#[cfg(test)]
mod casing_tests {
    use super::*;
    use crate::evaluation::{EvalLimits, NoBindings};
    use tablecraft_core::limits::{Resource, ResourceBudget};
    #[test]
    fn casing_output_expansion_source_context_and_cancellation_are_bounded() {
        let locale = "und".parse().unwrap();
        let args = [Value::Text("İ".repeat(5))];
        let mut budget = ResourceBudget::new(1000, 0, 10000);
        let mut evaluator = Evaluator::new(
            &NoBindings,
            &mut budget,
            EvalLimits {
                max_text_bytes: 14,
                ..Default::default()
            },
        )
        .with_case_locale(&locale);
        assert_eq!(
            evaluator.apply_text_function(TextFunction::LCase, &args),
            Err(EvalError::TextLimit)
        );
        let mut budget = ResourceBudget::new(1, 0, 1000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_case_locale(&locale);
        assert!(
            matches!(evaluator.apply_text_function(TextFunction::LCase,&[Value::Text("ABC".into())]),Err(EvalError::Limit(error)) if error.resource==Resource::Bytes)
        );
        let mut budget = ResourceBudget::new(1000, 0, 10);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_case_locale(&locale);
        assert!(
            matches!(evaluator.apply_text_function(TextFunction::LCase,&args),Err(EvalError::Limit(error)) if error.resource==Resource::Steps)
        );
        let mut budget = ResourceBudget::new(1000, 0, 10000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_case_locale(&locale)
            .with_cancellation(&|| true);
        assert_eq!(
            evaluator.apply_text_function(TextFunction::LCase, &args),
            Err(EvalError::Cancelled)
        );
    }
}
