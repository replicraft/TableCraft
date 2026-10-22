//! Exact integer bit patterns, separate from predicate truth and rounded coercions.
use super::{Truth, binary_conversion, coercion};
use crate::{EvalError, functions::conversions::Conversion, numeric};
use tablecraft_core::values::Value;

/// Eager logical operator over checked materialized numeric operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logical {
    /// Bitwise conjunction.
    And,
    /// Bitwise disjunction.
    Or,
    /// Bitwise exclusive disjunction.
    Xor,
    /// Bitwise equivalence.
    Equivalence,
    /// Bitwise material implication.
    Implication,
}

impl Logical {
    /// Preserve Boolean/NULL truth tables, or combine exact integers and NULL.
    /// Mixed Boolean/integer operands use signed TRUE=-1. Numeric/NULL follows
    /// the documented integral tables. Float/fixed/legacy DATE operands use
    /// checked half-even Long coercion, promoted by an Int64 operand to LongLong.
    /// Text requires the evaluator's explicit number policy instead.
    pub fn apply(self, left: &Value, right: &Value) -> Result<Value, EvalError> {
        if let Some(conversion) = binary_conversion(left, right) {
            let left = coercion::convert(left, conversion)?;
            let right = coercion::convert(right, conversion)?;
            return self.apply_integers(&left, &right);
        }
        self.apply_integers(left, right)
    }

    fn apply_integers(self, left: &Value, right: &Value) -> Result<Value, EvalError> {
        if matches!(left, Value::Boolean(_) | Value::Null)
            && matches!(right, Value::Boolean(_) | Value::Null)
        {
            let left = Truth::from_value(left)?;
            let right = Truth::from_value(right)?;
            return Ok(match self {
                Self::And => left.and(right),
                Self::Or => left.or(right),
                Self::Xor => left.xor(right),
                Self::Equivalence => left.equivalent(right),
                Self::Implication => left.implies(right),
            }
            .into_value());
        }
        let integer = |value: &Value| {
            if matches!(value, Value::Null) {
                Ok(None)
            } else {
                numeric::integer(value)
                    .map(Some)
                    .ok_or(EvalError::TypeMismatch)
            }
        };
        let a = integer(left)?;
        let b = integer(right)?;
        let bits = match (self, a, b) {
            (Self::And, Some(a), Some(b)) => a & b,
            (Self::Or, Some(a), Some(b)) => a | b,
            (Self::Xor, Some(a), Some(b)) => a ^ b,
            (Self::Equivalence, Some(a), Some(b)) => !(a ^ b),
            (Self::Implication, Some(a), Some(b)) => !a | b,
            (Self::And, Some(0), None) | (Self::And, None, Some(0)) => 0,
            (Self::Or, Some(a), None) | (Self::Or, None, Some(a)) => a,
            (Self::Implication, Some(a), None) if a != -1 => !a,
            (Self::Implication, None, Some(b)) if b != 0 => b,
            _ => return Ok(Value::Null),
        };
        // Integral inputs are already sign-extended. Bitwise results retain
        // that extension at the chosen width; narrowing restores its carrier
        // and intentionally discards complemented high bits for Byte.
        Ok(match (left, right) {
            (Value::Int64(_), _) | (_, Value::Int64(_)) => Value::Int64(bits),
            (Value::Int32(_), _) | (_, Value::Int32(_)) => Value::Int32(bits as i32),
            (Value::Byte(_), Value::Byte(_) | Value::Null) | (Value::Null, Value::Byte(_)) => {
                Value::Byte(bits as u8)
            }
            _ => Value::Int16(bits as i16),
        })
    }
}

/// Complement a Boolean or exact integer, retaining its carrier and bit width.
/// Float/fixed/legacy DATE operands first undergo checked half-even Long
/// conversion. NULL stays NULL; Text requires an explicit evaluator policy.
pub fn complement(value: &Value) -> Result<Value, EvalError> {
    if coercion::rounded(value) {
        let value = coercion::convert(value, Conversion::Long)?;
        return complement(&value);
    }
    Ok(match value {
        Value::Null => Value::Null,
        Value::Boolean(value) => Value::Boolean(!value),
        Value::Byte(value) => Value::Byte(!value),
        Value::Int16(value) => Value::Int16(!value),
        Value::Int32(value) => Value::Int32(!value),
        Value::Int64(value) => Value::Int64(!value),
        _ => return Err(EvalError::TypeMismatch),
    })
}

#[cfg(test)]
mod tests;
