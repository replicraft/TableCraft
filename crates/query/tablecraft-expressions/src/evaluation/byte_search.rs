//! InStrB consumes raw strings in written order without Unicode publication.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode},
    functions::text::{
        TextFunction,
        byte_string::{self, ByteOperand, ByteString, ExpressionByteOperand},
        sql_arguments::{self, Argument},
    },
};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};
use tablecraft_sql::select::FunctionArgument;

impl Evaluator<'_> {
    pub(super) fn byte_search_call(
        &mut self,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<Value, EvalError> {
        let mut arguments = sql_arguments::normalize(TextFunction::InStrB, arguments)
            .ok_or(EvalError::UnsupportedExpression)?;
        let count = arguments.len();
        let start = if count > 2 {
            Some(
                match arguments.next().ok_or(EvalError::UnsupportedExpression)? {
                    Argument::Expression(expression) => {
                        self.expression_operand(expression, depth + 1)?
                    }
                    Argument::Integer(value) => ExpressionValue::Value(Value::Int64(value)),
                    Argument::EmptyText => ExpressionValue::Value(Value::Text(String::new())),
                },
            )
        } else {
            None
        };
        let Some(Argument::Expression(haystack)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };
        let haystack = self.expression_byte_operand(haystack, depth + 1)?;
        let Some(Argument::Expression(needle)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };
        let needle = self.expression_byte_operand(needle, depth + 1)?;
        let mode = match arguments.next() {
            Some(Argument::Expression(expression)) => {
                Some(self.expression_operand(expression, depth + 1)?)
            }
            Some(Argument::Integer(value)) => Some(ExpressionValue::Value(Value::Int64(value))),
            Some(Argument::EmptyText) => Some(ExpressionValue::Value(Value::Text(String::new()))),
            None => None,
        };
        self.apply_expression_byte_search(start.as_ref(), haystack, needle, mode.as_ref())
    }

    /// Admit mode, start, NULL and each source in order after eager evaluation.
    pub fn apply_expression_byte_search(
        &mut self,
        start: Option<&ExpressionValue>,
        haystack: ExpressionByteOperand,
        needle: ExpressionByteOperand,
        mode: Option<&ExpressionValue>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let empty_number = Value::Int32(0);
        let mode = mode
            .map(|value| text_arguments::scalar(value, &empty_number))
            .transpose()?;
        let mode = self.byte_search_mode(mode)?;
        let start = start
            .map(|value| text_arguments::scalar(value, &empty_number))
            .transpose()?;
        let start = self.byte_search_start(start)?;
        let null_source = haystack.is_null() || needle.is_null();
        self.byte_search_inputs(
            start,
            mode,
            null_source,
            || haystack.into_string(),
            || needle.into_string(),
        )
    }

    /// Apply InStrB to written-order evaluated arguments. Binary search is
    /// supported; text/database/LCID byte-search providers remain unavailable.
    pub fn apply_byte_search(
        &mut self,
        start: Option<&Value>,
        haystack: ByteOperand,
        needle: ByteOperand,
        mode: Option<&Value>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        let mode = self.byte_search_mode(mode)?;
        let start = self.byte_search_start(start)?;
        let null_source = matches!(&haystack, ByteOperand::Scalar(Value::Null))
            || matches!(&needle, ByteOperand::Scalar(Value::Null));
        self.byte_search_inputs(start, mode, null_source, || Ok(haystack), || Ok(needle))
    }

    fn byte_search_mode(
        &mut self,
        mode: Option<&Value>,
    ) -> Result<Option<StringCompareMode>, EvalError> {
        let normalized_mode = self.text_long_override(mode)?;
        byte_string::search_mode(normalized_mode.as_ref().or(mode))
    }

    fn byte_search_start(&mut self, start: Option<&Value>) -> Result<i32, EvalError> {
        let start = start
            .map(|value| self.byte_long(value))
            .transpose()?
            .unwrap_or(1);
        if start < 1 {
            return Err(EvalError::Domain);
        }
        Ok(start)
    }

    fn byte_search_inputs(
        &mut self,
        start: i32,
        mode: Option<StringCompareMode>,
        null_source: bool,
        haystack: impl FnOnce() -> Result<ByteOperand, EvalError>,
        needle: impl FnOnce() -> Result<ByteOperand, EvalError>,
    ) -> Result<Value, EvalError> {
        if null_source {
            return Ok(Value::Null);
        }
        let haystack = self.search_bytes(haystack()?)?;
        let needle = self.search_bytes(needle()?)?;
        if haystack.is_empty() || start > haystack.len() {
            return Ok(Value::Int32(0));
        }
        if needle.is_empty() {
            return Ok(Value::Int32(start));
        }
        let mode = mode
            .or_else(|| self.collation.and_then(Collation::option_compare))
            .ok_or(EvalError::CollationRequired)?;
        if mode != StringCompareMode::Binary {
            return Err(EvalError::UnsupportedExpression);
        }
        let position = byte_string::find_binary(&haystack, &needle, start, || {
            if self.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
            Ok(())
        })?;
        Ok(Value::Int32(position))
    }

    fn search_bytes(&mut self, operand: ByteOperand) -> Result<ByteString, EvalError> {
        let operand = self.coerce_byte_string_source(operand)?;
        match operand {
            ByteOperand::Bytes(bytes) => Ok(bytes),
            ByteOperand::Scalar(Value::Text(text)) => self.encode_byte_string(&text),
            ByteOperand::Scalar(_) => Err(EvalError::TypeMismatch),
        }
    }
}
