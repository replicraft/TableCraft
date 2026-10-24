//! Contextual scalar predicates, separate from strict set/group key comparison.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::{self, Comparison},
    functions::conversions::Conversion,
    logic::Truth,
};
use tablecraft_core::values::{ExpressionValue, Value};

mod dates;
mod effective;
mod empty;
mod floating;
mod strings;

#[cfg(test)]
mod raw_tests;

impl Evaluator<'_> {
    /// Compare expression operands without implicitly converting Error codes.
    /// Standard Error/Error pairs compare their codes; mixed kinds are TypeMismatch.
    /// Raw String/NULL yields Unknown without Unicode publication. String pairs
    /// use an explicitly selected raw-unit provider or strict materialization.
    /// Host Empty selects numeric zero or empty String under its counterpart's
    /// comparison profile. Opaque codes and Missing retain explicit refusal.
    pub fn compare_expression_values(
        &mut self,
        operator: Comparison,
        left: &ExpressionValue,
        right: &ExpressionValue,
    ) -> Result<Truth, EvalError> {
        if let Some(result) = self.compare_empty_values(operator, left, right)? {
            return Ok(result);
        }
        if let Some(result) = self.compare_retained_strings(operator, left, right)? {
            return Ok(result);
        }

        match (left, right) {
            (ExpressionValue::Value(left), ExpressionValue::Value(right)) => {
                self.compare_values(operator, left, right)
            }
            (ExpressionValue::StringBytes(_), ExpressionValue::Value(Value::Null))
            | (ExpressionValue::Value(Value::Null), ExpressionValue::StringBytes(_)) => {
                self.check(0)?;
                Ok(Truth::Unknown)
            }
            (ExpressionValue::StringBytes(left), ExpressionValue::StringBytes(right)) => {
                let left = self.publish_expression_string(left)?;
                let right = self.publish_expression_string(right)?;
                self.compare_values(operator, &left, &right)
            }
            (ExpressionValue::StringBytes(left), ExpressionValue::Value(right)) => {
                let left = self.publish_expression_string(left)?;
                self.compare_values(operator, &left, right)
            }
            (ExpressionValue::Value(left), ExpressionValue::StringBytes(right)) => {
                let right = self.publish_expression_string(right)?;
                self.compare_values(operator, left, &right)
            }
            (ExpressionValue::Error(left), ExpressionValue::Error(right)) => {
                self.check(0)?;
                let left = u16::try_from(*left).map_err(|_| EvalError::UnsupportedExpression)?;
                let right = u16::try_from(*right).map_err(|_| EvalError::UnsupportedExpression)?;
                comparison::compare(
                    operator,
                    &Value::Int32(i32::from(left)),
                    &Value::Int32(i32::from(right)),
                    None,
                )
            }
            (ExpressionValue::Error(_), _) | (_, ExpressionValue::Error(_)) => {
                self.check(0)?;
                Err(EvalError::TypeMismatch)
            }
            _ => {
                self.check(0)?;
                Err(EvalError::NonScalarExpression)
            }
        }
    }

    /// Compare already-evaluated scalar operands under the explicit numeric/date
    /// predicate profile. Text/Text retains collation; numeric/Text uses NumberFormat
    /// and legacy DATE/Text uses the existing CDate policy. Legacy DATE/numeric
    /// uses checked CDate conversion unless Decimal selects raw serial comparison.
    /// Declared/Variant types are not inferred; set/group keys keep strict comparison.
    pub fn compare_values(
        &mut self,
        operator: Comparison,
        left: &Value,
        right: &Value,
    ) -> Result<Truth, EvalError> {
        self.check(0)?;
        if matches!(left, Value::Null) || matches!(right, Value::Null) {
            return Ok(Truth::Unknown);
        }
        if let Some(result) = self.compare_effective_date_values(operator, left, right)? {
            return Ok(result);
        }
        if let Some(result) = self.compare_numeric_pair(operator, left, right)? {
            return Ok(result);
        }
        if let Some(result) = self.compare_floating_integer(operator, left, right)? {
            return Ok(result);
        }
        let converted_left = self.converted_comparison_operand(left, right)?;
        let converted_right = self.converted_comparison_operand(right, left)?;
        let result = comparison::compare(
            operator,
            converted_left.as_ref().unwrap_or(left),
            converted_right.as_ref().unwrap_or(right),
            self.collation,
        )?;
        if matches!((left, right), (Value::Text(_), Value::Text(_))) {
            self.check(0)?;
        }

        Ok(result)
    }
}

fn target(value: &Value, other: &Value) -> Option<Conversion> {
    match (value, other) {
        (Value::Text(_), Value::Currency(_))
        | (Value::Single(_) | Value::Double(_), Value::Currency(_)) => Some(Conversion::Currency),
        (Value::Text(_), Value::Decimal(_))
        | (Value::Single(_) | Value::Double(_), Value::Decimal(_)) => Some(Conversion::Decimal),
        (
            Value::Text(_),
            Value::Byte(_)
            | Value::Boolean(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Single(_)
            | Value::Double(_),
        ) => Some(Conversion::Double),
        _ => None,
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod error_tests;
