//! Bounded text concatenation; locale-dependent numeric/date rendering is deferred.
use crate::EvalError;
use tablecraft_core::values::Value;

/// Concatenate Text/NULL with `&`: one NULL becomes empty; two NULLs stay NULL.
pub fn concatenate(left: &Value, right: &Value, max_bytes: usize) -> Result<Value, EvalError> {
    if matches!((left, right), (Value::Null, Value::Null)) {
        return Ok(Value::Null);
    }
    join(text(left)?, text(right)?, max_bytes)
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;
    proptest! {
        #![proptest_config(crate::test_support::config())]
        #[test]
        fn unicode_concatenation_preserves_contents_and_respects_bytes(
            a in prop::collection::vec(any::<char>(), 0..64),
            b in prop::collection::vec(any::<char>(), 0..64),
        ) {
            let (a, b): (String, String) = (a.into_iter().collect(), b.into_iter().collect());
            let expected = format!("{a}{b}");
            let (a, b) = (Value::Text(a), Value::Text(b));
            prop_assert_eq!(concatenate(&a, &b, expected.len()), Ok(Value::Text(expected.clone())));
            if !expected.is_empty() {
                prop_assert_eq!(concatenate(&a, &b, expected.len() - 1), Err(EvalError::TextLimit));
            }
        }
    }
}
fn text(value: &Value) -> Result<&str, EvalError> {
    match value {
        Value::Null => Ok(""),
        Value::Text(value) => Ok(value),
        _ => Err(EvalError::TypeMismatch),
    }
}
/// Concatenate Text/NULL with `+`: either NULL makes the result NULL.
pub fn plus(left: &Value, right: &Value, max_bytes: usize) -> Result<Value, EvalError> {
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    match (left, right) {
        (Value::Text(left), Value::Text(right)) => join(left, right, max_bytes),
        _ => Err(EvalError::TypeMismatch),
    }
}
fn join(left: &str, right: &str, max_bytes: usize) -> Result<Value, EvalError> {
    let length = left
        .len()
        .checked_add(right.len())
        .ok_or(EvalError::TextLimit)?;
    if length > max_bytes {
        return Err(EvalError::TextLimit);
    }
    let mut result = String::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| EvalError::Allocation)?;
    result.push_str(left);
    result.push_str(right);
    Ok(Value::Text(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concatenation_operators_have_distinct_null_behavior_and_byte_limits() {
        let text = Value::Text("é".into());
        assert_eq!(concatenate(&text, &Value::Null, 2), Ok(text.clone()));
        assert_eq!(concatenate(&Value::Null, &text, 2), Ok(text.clone()));
        assert_eq!(concatenate(&Value::Null, &Value::Null, 0), Ok(Value::Null));
        assert_eq!(plus(&text, &Value::Null, 2), Ok(Value::Null));
        assert_eq!(plus(&Value::Null, &text, 2), Ok(Value::Null));
        assert_eq!(concatenate(&text, &text, 3), Err(EvalError::TextLimit));
        assert_eq!(
            concatenate(&Value::Text(String::new()), &Value::Null, 0),
            Ok(Value::Text(String::new()))
        );
        assert_eq!(
            concatenate(&Value::Int32(1), &text, 10),
            Err(EvalError::TypeMismatch)
        );
    }
}
