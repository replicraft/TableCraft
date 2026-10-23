//! InStrB consumes raw strings in written order without Unicode publication.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::{Collation, StringCompareMode},
    functions::text::{
        TextFunction,
        byte_string::{self, ByteOperand, ByteString},
        sql_arguments::{self, Argument},
    },
};
use tablecraft_core::{limits::Resource, values::Value};
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
                    Argument::Expression(expression) => self.node(expression, depth + 1)?,
                    Argument::Integer(value) => Value::Int64(value),
                    Argument::EmptyText => Value::Text(String::new()),
                },
            )
        } else {
            None
        };
        let Some(Argument::Expression(haystack)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };
        let haystack = self.byte_operand(haystack, depth + 1)?;
        let Some(Argument::Expression(needle)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };
        let needle = self.byte_operand(needle, depth + 1)?;
        let mode = match arguments.next() {
            Some(Argument::Expression(expression)) => Some(self.node(expression, depth + 1)?),
            Some(Argument::Integer(value)) => Some(Value::Int64(value)),
            Some(Argument::EmptyText) => Some(Value::Text(String::new())),
            None => None,
        };
        self.apply_byte_search(start.as_ref(), haystack, needle, mode.as_ref())
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
        let normalized_mode = self.text_long_override(mode)?;
        let mode = byte_string::search_mode(normalized_mode.as_ref().or(mode))?;
        let start = start
            .map(|value| self.byte_long(value))
            .transpose()?
            .unwrap_or(1);
        if start < 1 {
            return Err(EvalError::Domain);
        }
        if matches!(&haystack, ByteOperand::Scalar(Value::Null))
            || matches!(&needle, ByteOperand::Scalar(Value::Null))
        {
            return Ok(Value::Null);
        }
        let haystack = self.search_bytes(haystack)?;
        let needle = self.search_bytes(needle)?;
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
