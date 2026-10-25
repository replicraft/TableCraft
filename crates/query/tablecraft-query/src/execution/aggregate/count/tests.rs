use super::*;
use tablecraft_core::values::ExpressionString;

#[test]
fn count_nullness_keeps_empty_missing_and_error_states_distinct() {
    assert_eq!(
        counted_value(ExpressionByteOperand::Scalar(ExpressionValue::Value(
            Value::Null
        ))),
        Ok(false)
    );
    for value in [ExpressionValue::Empty, ExpressionValue::Missing] {
        assert_eq!(
            counted_value(ExpressionByteOperand::Scalar(value)),
            Err(EvalError::NonScalarExpression)
        );
    }
    for value in [
        ExpressionValue::Value(Value::Text(String::new())),
        ExpressionValue::Error(7),
        ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216])),
        ExpressionValue::StringBytes(ExpressionString::from_owned(vec![65])),
    ] {
        assert_eq!(
            counted_value(ExpressionByteOperand::Scalar(value)),
            Ok(true)
        );
    }
}
