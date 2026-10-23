//! The explicit Variant String/numeric exception, without regional coercion.
use super::{Comparison, from_ordering};
use crate::{EvalError, logic::Truth, numeric::OperandDeclaration};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

/// Apply the reviewed String/numeric ordering only when both declarations are
/// Variant. Numeric values precede Strings, irrespective of their text contents.
/// Returns None for all other pairs so their owning comparison policy can run.
/// Boolean and Date are not numeric types for this exception. Non-finite floating
/// inputs retain explicit rejection rather than acquiring a category ordering.
/// No text scan, conversion, locale lookup or allocation occurs here.
pub fn variant_text_numeric(
    operator: Comparison,
    left: &Value,
    right: &Value,
    left_declaration: OperandDeclaration,
    right_declaration: OperandDeclaration,
) -> Result<Option<Truth>, EvalError> {
    if left_declaration != OperandDeclaration::Variant
        || right_declaration != OperandDeclaration::Variant
    {
        return Ok(None);
    }
    let order = match (left, right) {
        (Value::Text(_), numeric) if is_numeric(numeric)? => Ordering::Greater,
        (numeric, Value::Text(_)) if is_numeric(numeric)? => Ordering::Less,
        _ => return Ok(None),
    };
    Ok(Some(from_ordering(operator, order)))
}

fn is_numeric(value: &Value) -> Result<bool, EvalError> {
    Ok(match value {
        Value::Byte(_)
        | Value::Int16(_)
        | Value::Int32(_)
        | Value::Int64(_)
        | Value::Currency(_)
        | Value::Decimal(_) => true,
        Value::Single(value) => {
            if !value.to_float().is_finite() {
                return Err(EvalError::NonFinite);
            }
            true
        }
        Value::Double(value) => {
            if !value.to_float().is_finite() {
                return Err(EvalError::NonFinite);
            }
            true
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests;
