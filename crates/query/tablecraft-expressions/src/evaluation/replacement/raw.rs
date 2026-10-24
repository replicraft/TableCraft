//! Lossless Replace operands share control conversion and bounded output.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    comparison::{StringCompareMode, StringInput, StringSearchProfile},
    evaluation::{byte_output, text_output},
    functions::text::{
        TextFunction,
        byte_string::{ByteOperand, ByteString},
        sql_arguments::{self, Argument},
    },
};
use std::borrow::Cow;
use tablecraft_core::values::{ExpressionString, ExpressionValue, Value};
use tablecraft_sql::select::FunctionArgument;

enum Source<'a> {
    Raw(&'a ExpressionString),
    Text(Cow<'a, str>),
}

impl<'a> Source<'a> {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Raw(_) => None,
        }
    }

    fn input(&self) -> StringInput<'_> {
        match self {
            Self::Raw(bytes) => StringInput::Utf16Le(bytes.as_bytes()),
            Self::Text(text) => StringInput::Text(text),
        }
    }

    fn unicode(self, evaluator: &mut Evaluator<'_>) -> Result<Cow<'a, str>, EvalError> {
        match self {
            Self::Text(text) => Ok(text),
            Self::Raw(bytes) => match evaluator.publish_expression_string(bytes)? {
                Value::Text(text) => Ok(Cow::Owned(text)),
                _ => Err(EvalError::TypeMismatch),
            },
        }
    }
}

impl Evaluator<'_> {
    pub(in crate::evaluation) fn replacement_byte_call(
        &mut self,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<ByteOperand, EvalError> {
        let arguments = sql_arguments::normalize(TextFunction::Replace, arguments)
            .ok_or(EvalError::UnsupportedExpression)?;
        let count = arguments.len();
        let mut values: [ExpressionValue; TextFunction::MAX_ARGUMENTS] =
            std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
        for (index, (argument, value)) in arguments.zip(&mut values).enumerate() {
            *value = match argument {
                Argument::Expression(expression) if index < 3 => self
                    .expression_byte_operand(expression, depth + 1)?
                    .into_expression_value(),
                Argument::Expression(expression) => {
                    self.expression_operand(expression, depth + 1)?
                }
                Argument::Integer(value) => ExpressionValue::Value(Value::Int64(value)),
                Argument::EmptyText => ExpressionValue::Value(Value::Text(String::new())),
            };
        }
        self.apply_retained_replacement(&values[..count])
    }

    /// Retain String units until a raw-aware consumer or strict publication.
    /// Written arguments must already be evaluated eagerly. Ordinary Unicode
    /// results keep the borrowed recipe and its existing UTF-8 output accounting.
    pub fn apply_retained_replacement(
        &mut self,
        arguments: &[ExpressionValue],
    ) -> Result<ByteOperand, EvalError> {
        if !TextFunction::Replace.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(arguments)?;
        let empty_number = Value::Int32(0);
        let options = self.replacement_controls(arguments.len(), |index| {
            text_arguments::scalar(&arguments[index], &empty_number)
        })?;

        let source = self.replacement_source(&arguments[0])?;
        let find = self.replacement_source(&arguments[1])?;
        let replacement = self.replacement_source(&arguments[2])?;
        if let (Some(source), Some(find), Some(replacement)) =
            (source.text(), find.text(), replacement.text())
            && options.text_suffix(source, &mut || self.check(0))?
        {
            let mut output = text_output::Output {
                budget: self.budget,
                cancelled: self.cancelled,
                max_bytes: self.limits.max_text_bytes,
                text: String::new(),
            };
            options
                .with_texts(source, find, replacement)
                .write(self.collation, &mut output)?;
            return Ok(ByteOperand::Scalar(Value::Text(output.text)));
        }

        let source_units = source.input().units(self.budget, self.cancelled)?;
        let find_units = find.input().units(self.budget, self.cancelled)?;
        let replacement_units = replacement.input().units(self.budget, self.cancelled)?;
        if options.needs_search(source_units.len(), find_units.len()) {
            let mode = options.selected_mode(self.collation)?;
            let supports_units = mode == StringCompareMode::Binary
                || self.collation.is_some_and(|provider| {
                    provider.string_search_profile(mode) == StringSearchProfile::Utf16
                });
            if !supports_units {
                let source = source.unicode(self)?;
                let find = find.unicode(self)?;
                let replacement = replacement.unicode(self)?;
                let mut output = text_output::Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                options
                    .with_texts(&source, &find, &replacement)
                    .write(self.collation, &mut output)?;
                return Ok(ByteOperand::Scalar(Value::Text(output.text)));
            }
        }

        let mut output = byte_output::Output {
            budget: self.budget,
            cancelled: self.cancelled,
            max_bytes: i32::MAX as usize,
            bytes: Vec::new(),
        };
        options.write_utf16(
            &source_units,
            &find_units,
            &replacement_units,
            self.collation,
            &mut output,
        )?;
        ByteString::from_owned(output.bytes).map(ByteOperand::Bytes)
    }

    fn replacement_source<'a>(
        &mut self,
        value: &'a ExpressionValue,
    ) -> Result<Source<'a>, EvalError> {
        match value {
            ExpressionValue::StringBytes(bytes) => Ok(Source::Raw(bytes)),
            ExpressionValue::Empty => Ok(Source::Text(Cow::Borrowed(""))),
            ExpressionValue::Value(Value::Null) => Err(EvalError::InvalidNullConversion),
            ExpressionValue::Value(Value::Text(text)) => Ok(Source::Text(Cow::Borrowed(text))),
            value => {
                let empty = Value::Text(String::new());
                let value = text_arguments::scalar(value, &empty)?;
                match self.string_operand(value)? {
                    Some(Value::Text(text)) => Ok(Source::Text(Cow::Owned(text))),
                    _ => Err(EvalError::TypeMismatch),
                }
            }
        }
    }
}
