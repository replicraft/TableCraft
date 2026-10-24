//! Raw StrComp operands use explicit binary/database unit comparison contracts.
use super::{Evaluator, compare, text_arguments};
use crate::{
    EvalError,
    comparison::{
        Collation, StringCompareMode, StringComparisonProfile, StringInput, order_utf16_strings,
    },
    functions::text::{
        TextFunction, TextResult,
        byte_string::{ByteOperand, ExpressionByteOperand, compare_binary_utf16},
        sql_arguments::{self, Argument},
    },
};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};
use tablecraft_sql::select::FunctionArgument;

impl Evaluator<'_> {
    pub(in crate::evaluation) fn comparison_byte_call(
        &mut self,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<Value, EvalError> {
        let mut arguments = sql_arguments::normalize(TextFunction::StrComp, arguments)
            .ok_or(EvalError::UnsupportedExpression)?;
        let Some(Argument::Expression(left)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };
        let Some(Argument::Expression(right)) = arguments.next() else {
            return Err(EvalError::UnsupportedExpression);
        };

        let left = self.expression_byte_operand(left, depth + 1)?;
        let right = self.expression_byte_operand(right, depth + 1)?;
        let option = match arguments.next() {
            Some(Argument::Expression(value)) => Some(self.expression_operand(value, depth + 1)?),
            None => None,
            _ => return Err(EvalError::UnsupportedExpression),
        };
        self.apply_expression_byte_comparison(left, right, option)
    }

    /// Compare eager String operands without prematurely publishing raw UTF-16.
    /// Mode admission precedes NULL propagation. Binary compares complete units;
    /// Database can use an explicitly selected raw-unit provider. Other provider
    /// modes retain strict Unicode publication and their existing policies.
    pub fn apply_expression_byte_comparison(
        &mut self,
        left: ExpressionByteOperand,
        right: ExpressionByteOperand,
        option: Option<ExpressionValue>,
    ) -> Result<Value, EvalError> {
        let (left, right) = match (left, right) {
            (ExpressionByteOperand::Scalar(left), ExpressionByteOperand::Scalar(right))
                if !matches!(&left, ExpressionValue::StringBytes(_))
                    && !matches!(&right, ExpressionValue::StringBytes(_)) =>
            {
                let count = if option.is_some() { 3 } else { 2 };
                let arguments = [
                    left,
                    right,
                    option.unwrap_or(ExpressionValue::Value(Value::Null)),
                ];
                return self.apply_expression_comparison(&arguments[..count]);
            }
            operands => operands,
        };
        for value in [&left, &right] {
            if let ExpressionByteOperand::Scalar(value) = value {
                self.text_expression_limits(std::slice::from_ref(value))?;
            }
        }
        if let Some(option) = &option {
            self.text_expression_limits(std::slice::from_ref(option))?;
        }
        self.check(0)?;

        let empty_number = Value::Int32(0);
        let option = option
            .as_ref()
            .map(|value| text_arguments::scalar(value, &empty_number))
            .transpose()?;
        let normalized = self.text_long_override(option)?;
        let mode = compare::mode(normalized.as_ref().or(option), None)?;
        if left.is_null() || right.is_null() {
            return Ok(Value::Null);
        }

        let left = self.coerce_byte_string_source(left.into_string()?)?;
        let right = self.coerce_byte_string_source(right.into_string()?)?;
        let selected = mode.or_else(|| self.collation.and_then(Collation::option_compare));
        if selected == Some(StringCompareMode::Binary) {
            let order = compare_binary_utf16(&left, &right, &mut || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            })?;
            return Ok(comparison_value(order));
        }

        if selected == Some(StringCompareMode::Database)
            && let Some(collation) = self.collation
            && collation.string_comparison_profile() == StringComparisonProfile::Utf16
        {
            let order = order_utf16_strings(
                comparison_input(&left)?,
                comparison_input(&right)?,
                collation,
                self.budget,
                self.cancelled,
            )?;
            return Ok(comparison_value(order));
        }

        let left = self.publish_byte_operand(left)?;
        let right = self.publish_byte_operand(right)?;
        let result = compare::apply_operands(&left, &right, mode, self.collation, &mut || {
            if self.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
            Ok(())
        })?;
        match result {
            TextResult::Comparison(value) => Ok(Value::Int16(value)),
            TextResult::Null => Ok(Value::Null),
            _ => Err(EvalError::UnsupportedExpression),
        }
    }
}

fn comparison_input(value: &ByteOperand) -> Result<StringInput<'_>, EvalError> {
    match value {
        ByteOperand::Scalar(Value::Text(text)) => Ok(StringInput::Text(text)),
        ByteOperand::Bytes(bytes) => Ok(StringInput::Utf16Le(bytes.as_bytes())),
        _ => Err(EvalError::TypeMismatch),
    }
}

fn comparison_value(order: Ordering) -> Value {
    Value::Int16(match order {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    })
}
