//! Strict stored-scalar ordering shared by query keys and native view services.
//!
//! These functions do not coerce strings into numbers or reinterpret stored
//! representations as equality. Callers own cumulative traversal/comparison
//! quotas and cancellation; supplied text providers own their callback quotas.
pub(crate) mod numeric;

use crate::QueryError;
use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    comparison::{self, Collation, Comparison},
    logic::Truth,
};

/// Validate each admitted stored key before sorting, including singleton sets.
/// Callers decide membership before invoking this service. Binary and GUID ordering
/// are not qualified by
/// this service. Text still needs an explicit qualified provider when compared.
pub fn validate_stored_key(value: &Value) -> Result<(), QueryError> {
    match value {
        Value::Single(value) if !value.to_float().is_finite() => Err(EvalError::NonFinite.into()),
        Value::Double(value) if !value.to_float().is_finite() => Err(EvalError::NonFinite.into()),
        Value::Binary(_) | Value::Guid(_) => Err(EvalError::UnsupportedExpression.into()),
        _ => Ok(()),
    }
}

/// Order admitted stored keys with NULL first and exact mixed numeric widening.
/// Descending callers reverse the result; equal keys retain their source order.
/// No source, host locale, Variant conversion or text-policy fallback is chosen.
pub fn compare_stored_keys(
    left: &Value,
    right: &Value,
    collation: Option<&dyn Collation>,
) -> Result<Ordering, QueryError> {
    validate_stored_key(left)?;
    validate_stored_key(right)?;
    match (left, right) {
        (Value::Null, Value::Null) => return Ok(Ordering::Equal),
        (Value::Null, _) => return Ok(Ordering::Less),
        (_, Value::Null) => return Ok(Ordering::Greater),
        _ => {}
    }
    if let Some(pair) = numeric::mixed_fixed_pair(left, right) {
        return pair.compare().map_err(Into::into);
    }
    match numeric::mixed_float_views(left, right) {
        Some((left, right)) => order_values(&left, &right, collation),
        None => order_values(left, right, collation),
    }
}

fn order_values(
    left: &Value,
    right: &Value,
    collation: Option<&dyn Collation>,
) -> Result<Ordering, QueryError> {
    if comparison::compare(Comparison::Equal, left, right, collation)? == Truth::True {
        return Ok(Ordering::Equal);
    }
    Ok(
        if comparison::compare(Comparison::Less, left, right, collation)? == Truth::True {
            Ordering::Less
        } else {
            Ordering::Greater
        },
    )
}

#[cfg(test)]
mod tests;
