//! Contextual numeric predicates, separate from strict set/group key comparison.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::{self, Comparison},
    functions::conversions::Conversion,
    logic::Truth,
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Compare already-evaluated scalar operands under the explicit numeric
    /// predicate profile. Text/Text retains collation; numeric/Text uses the
    /// execution's NumberFormat. This does not infer declared/Variant types or
    /// change the strict comparison service used by set/group keys.
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
        let left_target = target(left, right);
        let right_target = target(right, left);
        let converted_left = left_target
            .map(|target| self.apply_conversion(target, left))
            .transpose()?;
        let converted_right = right_target
            .map(|target| self.apply_conversion(target, right))
            .transpose()?;
        comparison::compare(
            operator,
            converted_left.as_ref().unwrap_or(left),
            converted_right.as_ref().unwrap_or(right),
            self.collation,
        )
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
