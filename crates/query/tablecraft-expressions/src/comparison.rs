//! SQL comparison, independent of representational Value equality.
use crate::{EvalError, logic::Truth, numeric};
use std::cmp::Ordering;
use tablecraft_core::values::Value;

/// Explicit text ordering supplied by the host; no implicit Rust/locale collation.
pub trait Collation {
    /// Compare decoded strings according to the selected database policy.
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError>;
}
/// SQL comparison operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// =.
    Equal,
    /// <>.
    NotEqual,
    /// <.
    Less,
    /// <=.
    LessOrEqual,
    /// >.
    Greater,
    /// >=.
    GreaterOrEqual,
}
/// Compare numeric values or strings with an explicitly supplied collation.
/// Either NULL yields unknown, including NULL = NULL.
pub fn compare(
    operator: Comparison,
    left: &Value,
    right: &Value,
    collation: Option<&dyn Collation>,
) -> Result<Truth, EvalError> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Truth::Unknown);
    }
    let order = match (left, right) {
        (Value::DateTime(left), Value::DateTime(right)) => left.temporal_cmp(*right),
        (Value::DateTime(_), _) | (_, Value::DateTime(_)) => return Err(EvalError::TypeMismatch),
        (Value::Text(left), Value::Text(right)) => collation
            .ok_or(EvalError::CollationRequired)?
            .compare(left, right)?,
        (Value::Text(_), _) | (_, Value::Text(_)) => return Err(EvalError::TypeMismatch),
        _ => numeric::compare(left, right)?,
    };
    let result = match operator {
        Comparison::Equal => order == Ordering::Equal,
        Comparison::NotEqual => order != Ordering::Equal,
        Comparison::Less => order == Ordering::Less,
        Comparison::LessOrEqual => order != Ordering::Greater,
        Comparison::Greater => order == Ordering::Greater,
        Comparison::GreaterOrEqual => order != Ordering::Less,
    };
    Ok(if result { Truth::True } else { Truth::False })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Ordinal;
    impl Collation for Ordinal {
        fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
            Ok(left.cmp(right))
        }
    }
    #[test]
    fn null_is_unknown_and_text_needs_a_selected_collation() {
        for op in [
            Comparison::Equal,
            Comparison::NotEqual,
            Comparison::Less,
            Comparison::LessOrEqual,
            Comparison::Greater,
            Comparison::GreaterOrEqual,
        ] {
            assert_eq!(
                compare(op, &Value::Null, &Value::Null, None),
                Ok(Truth::Unknown)
            );
        }
        let (a, b) = (Value::Text("A".into()), Value::Text("a".into()));
        assert_eq!(
            compare(Comparison::Equal, &a, &b, None),
            Err(EvalError::CollationRequired)
        );
        assert_eq!(
            compare(Comparison::Less, &a, &b, Some(&Ordinal)),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Equal, &Value::Byte(1), &Value::Int64(1), None),
            Ok(Truth::True)
        );
    }
}

#[cfg(test)]
mod temporal_tests {
    use super::*;
    use tablecraft_core::values::LegacyDateTime;
    fn date(days: f64) -> Value {
        Value::DateTime(LegacyDateTime::from_ole_days(days).unwrap())
    }
    #[test]
    fn legacy_dates_compare_temporally_without_numeric_or_text_coercion() {
        assert_eq!(
            compare(Comparison::Less, &date(-1.25), &date(-1.75), None),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Equal, &date(-0.25), &date(0.25), None),
            Ok(Truth::True)
        );
        assert_eq!(
            compare(Comparison::Equal, &date(1.0), &Value::Int64(1), None),
            Err(EvalError::TypeMismatch)
        );
        assert_eq!(
            compare(Comparison::Equal, &date(1.0), &Value::Null, None),
            Ok(Truth::Unknown)
        );
    }
}
