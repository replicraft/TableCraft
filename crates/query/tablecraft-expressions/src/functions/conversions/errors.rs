//! Variant Error construction and explicit code conversion; operational failures stay errors.
use super::Conversion;
use crate::EvalError;
use tablecraft_core::values::{ExpressionValue, Value};

const MAX_STANDARD_CODE: u32 = u16::MAX as u32;

/// Construct a standard error data value, or preserve an existing opaque Error.
/// Numeric inputs use checked Long conversion before the standard-code range check.
pub fn cverr(value: ExpressionValue) -> Result<ExpressionValue, EvalError> {
    let value = match value {
        ExpressionValue::Error(_) => return Ok(value),
        ExpressionValue::Value(Value::Null) => return Err(EvalError::InvalidNullConversion),
        ExpressionValue::Value(value) => value,
        _ => return Err(EvalError::TypeMismatch),
    };
    let Value::Int32(code) = Conversion::Long.apply(&value)? else {
        return Err(EvalError::TypeMismatch);
    };
    let code = u32::try_from(code).map_err(|_| EvalError::Domain)?;
    if code > MAX_STANDARD_CODE {
        return Err(EvalError::Domain);
    }
    Ok(ExpressionValue::Error(code))
}

/// Explicit numeric conversion of a standard error code. Unknown host HRESULT
/// mappings are rejected instead of interpreting them as signed or unsigned codes.
pub fn convert(conversion: Conversion, value: &ExpressionValue) -> Result<Value, EvalError> {
    match value {
        ExpressionValue::Value(value) => conversion.apply(value),
        ExpressionValue::Error(code) if *code <= MAX_STANDARD_CODE => {
            conversion.apply(&Value::Int64(i64::from(*code)))
        }
        ExpressionValue::Error(_) => Err(EvalError::UnsupportedExpression),
        _ => Err(EvalError::TypeMismatch),
    }
}

/// Publish ordinary scalars only. Error data never becomes stored NULL or a numeric code.
pub fn into_scalar(value: ExpressionValue) -> Result<Value, EvalError> {
    Value::try_from(value).map_err(|_| EvalError::NonScalarExpression)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tablecraft_core::values::{Double, ExpressionState, ValueError};

    #[test]
    fn construction_rounds_as_long_then_checks_the_standard_code_range() {
        for (input, expected) in [
            (0.0, 0),
            (2001.0, 2001),
            (-0.5, 0),
            (2.5, 2),
            (3.5, 4),
            (65534.5, 65534),
        ] {
            assert_eq!(
                cverr(ExpressionValue::Value(Value::Double(Double::from_float(
                    input
                )))),
                Ok(ExpressionValue::Error(expected))
            );
        }
        for value in [-1, 65536] {
            assert_eq!(
                cverr(ExpressionValue::Value(Value::Int32(value))),
                Err(EvalError::Domain)
            );
        }
        assert_eq!(
            cverr(ExpressionValue::Value(Value::Int64(i64::MAX))),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            cverr(ExpressionValue::Value(Value::Null)),
            Err(EvalError::InvalidNullConversion)
        );
        assert_eq!(
            cverr(ExpressionValue::Value(Value::Boolean(true))),
            Err(EvalError::Domain)
        );
        assert_eq!(
            cverr(ExpressionValue::Error(u32::MAX)),
            Ok(ExpressionValue::Error(u32::MAX))
        );
        assert_eq!(
            convert(Conversion::Long, &ExpressionValue::Error(u32::MAX)),
            Err(EvalError::UnsupportedExpression)
        );
    }

    #[test]
    fn explicit_conversion_respects_the_destination_and_never_grants_storage() {
        assert_eq!(
            convert(Conversion::Integer, &ExpressionValue::Error(2001)),
            Ok(Value::Int16(2001))
        );
        assert_eq!(
            convert(Conversion::Integer, &ExpressionValue::Error(65535)),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            convert(Conversion::Byte, &ExpressionValue::Error(256)),
            Err(EvalError::Overflow)
        );
        assert_eq!(
            convert(Conversion::Boolean, &ExpressionValue::Error(0)),
            Ok(Value::Boolean(false))
        );
        assert_eq!(
            Value::try_from(ExpressionValue::Error(0)),
            Err(ValueError::ExpressionNotStorable(ExpressionState::Error))
        );
        assert_eq!(
            into_scalar(ExpressionValue::Error(2001)),
            Err(EvalError::NonScalarExpression)
        );
    }

    proptest! {
        #![proptest_config(crate::test_support::config())]

        #[test]
        fn standard_codes_round_trip_only_through_explicit_long_conversion(code in any::<u16>()) {
            let value = cverr(ExpressionValue::Value(Value::Int32(i32::from(code)))).unwrap();
            prop_assert_eq!(&value, &ExpressionValue::Error(u32::from(code)));
            prop_assert_eq!(convert(Conversion::Long, &value).unwrap(), Value::Int32(i32::from(code)));
            prop_assert!(Value::try_from(value).is_err());
        }
    }
}
