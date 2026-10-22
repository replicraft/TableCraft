//! Materialized logical operands reuse the checked numeric conversion kernels.
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::Value;

pub(super) fn rounded(value: &Value) -> bool {
    matches!(
        value,
        Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
            | Value::DateTime(_)
            | Value::Text(_)
    )
}

pub(crate) fn binary_conversion(left: &Value, right: &Value) -> Option<Conversion> {
    if !rounded(left) && !rounded(right) {
        return None;
    }
    Some(
        if matches!(left, Value::Int64(_)) || matches!(right, Value::Int64(_)) {
            Conversion::LongLong
        } else {
            Conversion::Long
        },
    )
}

pub(super) fn convert(value: &Value, conversion: Conversion) -> Result<Value, EvalError> {
    if matches!(value, Value::Null) {
        Ok(Value::Null)
    } else {
        conversion.apply(value)
    }
}
