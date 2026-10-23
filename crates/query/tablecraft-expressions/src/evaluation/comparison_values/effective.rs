//! Explicit numeric effective types; declaration exceptions belong to the caller.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::{self, Comparison},
    functions::conversions::Conversion,
    logic::Truth,
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Compare under the reviewed numeric effective-type table. Numeric Text uses
    /// the selected numeric carrier's checked conversion and explicit NumberFormat.
    /// This does not infer declaration categories or implement the both-Variant
    /// String/numeric exception. Non-numeric pairs retain compare_values behavior;
    /// set/group key equality is separate. NULL retains Unknown before conversion.
    pub fn compare_effective_numeric_values(
        &mut self,
        operator: Comparison,
        left: &Value,
        right: &Value,
    ) -> Result<Truth, EvalError> {
        self.check(0)?;
        if matches!(left, Value::Null) || matches!(right, Value::Null) {
            return Ok(Truth::Unknown);
        }
        let Some(target) = target(left, right) else {
            return self.compare_values(operator, left, right);
        };
        let left = self.apply_conversion(target, left)?;
        let right = self.apply_conversion(target, right)?;
        comparison::compare(operator, &left, &right, None)
    }
}

fn numeric(value: &Value) -> bool {
    matches!(
        value,
        Value::Boolean(_)
            | Value::Byte(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
    )
}

fn target(left: &Value, right: &Value) -> Option<Conversion> {
    let text = |value: &Value| matches!(value, Value::Text(_));
    if !(numeric(left) && (numeric(right) || text(right)) || text(left) && numeric(right)) {
        return None;
    }
    let either = |matches: fn(&Value) -> bool| matches(left) || matches(right);
    Some(if either(|v| matches!(v, Value::Decimal(_))) {
        Conversion::Decimal
    } else if either(|v| matches!(v, Value::Currency(_))) {
        Conversion::Currency
    } else if either(|v| matches!(v, Value::Single(_))) {
        if either(|v| matches!(v, Value::Int32(_) | Value::Int64(_))) {
            Conversion::Double
        } else {
            Conversion::Single
        }
    } else if either(|v| matches!(v, Value::Double(_))) {
        Conversion::Double
    } else if either(|v| matches!(v, Value::Int64(_))) {
        Conversion::LongLong
    } else if either(|v| matches!(v, Value::Int32(_))) {
        Conversion::Long
    } else if either(|v| matches!(v, Value::Int16(_)))
        || (either(|v| matches!(v, Value::Byte(_))) && either(|v| matches!(v, Value::Boolean(_))))
    {
        Conversion::Integer
    } else if either(|v| matches!(v, Value::Boolean(_))) {
        Conversion::Boolean
    } else {
        Conversion::Byte
    })
}

#[cfg(test)]
mod tests;
