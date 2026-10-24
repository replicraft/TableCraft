//! Raw binary searches borrow active String ranges; provider searches publish Text.
use super::{Evaluator, search, text_arguments};
use crate::{
    EvalError,
    comparison::StringInput,
    functions::text::{TextFunction, TextResult},
};
use std::borrow::Cow;
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionString, ExpressionValue, Value},
};

enum Source<'a> {
    Raw(&'a ExpressionString),
    Text(Cow<'a, str>),
}

impl<'a> Source<'a> {
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
    pub(super) fn raw_search(
        &mut self,
        function: TextFunction,
        arguments: &[ExpressionValue],
        null_operand: bool,
    ) -> Result<Value, EvalError> {
        let empty_number = Value::Int32(0);
        let (options, start) = self.search_controls(function, arguments.len(), |index| {
            text_arguments::scalar(&arguments[index], &empty_number)
        })?;
        if null_operand {
            return Ok(Value::Null);
        }

        let (haystack, needle) = search::operand_indices(function, arguments.len());
        let result = if options.uses_binary(self.collation) {
            let haystack = self.search_source(&arguments[haystack])?;
            let needle = self.search_source(&arguments[needle])?;
            options.find_binary_inputs(start, haystack.input(), needle.input(), &mut || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            })?
        } else {
            let haystack = self.search_source(&arguments[haystack])?.unicode(self)?;
            let needle = self.search_source(&arguments[needle])?.unicode(self)?;
            options.find_text(start, &haystack, &needle, self.collation, &mut || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            })?
        };
        match result {
            TextResult::Position(value) => Ok(Value::Int32(value)),
            TextResult::Null => Ok(Value::Null),
            _ => Err(EvalError::UnsupportedExpression),
        }
    }

    fn search_source<'a>(&mut self, value: &'a ExpressionValue) -> Result<Source<'a>, EvalError> {
        if let ExpressionValue::StringBytes(bytes) = value {
            return Ok(Source::Raw(bytes));
        }
        match value {
            ExpressionValue::Empty => Ok(Source::Text(Cow::Borrowed(""))),
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
