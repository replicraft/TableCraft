//! Double-valued functions; exact fixed-point operations stay in their own paths.
use super::MathFunction;
use crate::{EvalError, functions::conversions::Conversion};
use tablecraft_core::values::{Double, Value};

pub(super) fn apply(function: MathFunction, value: &Value) -> Result<Value, EvalError> {
    if matches!(value, Value::Null) {
        return Ok(Value::Null);
    }
    let Value::Double(value) = Conversion::Double.apply(value)? else {
        return Err(EvalError::TypeMismatch);
    };
    let value = value.to_float();
    let result = match function {
        MathFunction::Sqr if value < 0.0 => return Err(EvalError::Domain),
        MathFunction::Log if value <= 0.0 => return Err(EvalError::Domain),
        MathFunction::Sqr => value.sqrt(),
        MathFunction::Exp => value.exp(),
        MathFunction::Log => value.ln(),
        MathFunction::Sin => value.sin(),
        MathFunction::Cos => value.cos(),
        MathFunction::Tan => value.tan(),
        MathFunction::Atn => value.atan(),
        _ => return Err(EvalError::UnsupportedExpression),
    };
    if !result.is_finite() {
        return Err(EvalError::Overflow);
    }
    Ok(Value::Double(Double::from_float(result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use tablecraft_core::values::{Currency, Decimal, Single};

    fn number(value: f64) -> Value {
        Value::Double(Double::from_float(value))
    }
    fn result(function: MathFunction, value: f64) -> f64 {
        let Value::Double(value) = function.apply(&[number(value)]).unwrap() else {
            panic!("Double required")
        };
        value.to_float()
    }
    #[test]
    fn all_numeric_carriers_convert_to_double_without_changing_exact_math_paths() {
        for value in [
            Value::Byte(4),
            Value::Int16(4),
            Value::Int32(4),
            Value::Int64(4),
            Value::Single(Single::from_float(4.0)),
            number(4.0),
            Value::Currency(Currency::from_scaled(40000)),
            Value::Decimal(Decimal::new(400, 2).unwrap()),
        ] {
            assert_eq!(MathFunction::Sqr.apply(&[value]), Ok(number(2.0)));
        }
        assert_eq!(
            MathFunction::Cos.apply(&[Value::Boolean(false)]),
            Ok(number(1.0))
        );
        assert_eq!(
            MathFunction::Sqr.apply(&[Value::Boolean(true)]),
            Err(EvalError::Domain)
        );
        assert_eq!(
            MathFunction::Sqr.apply(&[Value::Int64(i64::MAX)]),
            Ok(number((i64::MAX as f64).sqrt()))
        );
        assert_eq!(
            MathFunction::Log.apply(&[Value::Decimal(Decimal::new(1, 28).unwrap())]),
            Ok(number(1e-28_f64.ln()))
        );
    }
    #[test]
    fn null_nonfinite_domain_overflow_arity_and_wrong_carriers_are_explicit() {
        for function in [
            MathFunction::Sqr,
            MathFunction::Exp,
            MathFunction::Log,
            MathFunction::Sin,
            MathFunction::Cos,
            MathFunction::Tan,
            MathFunction::Atn,
        ] {
            assert_eq!(function.apply(&[Value::Null]), Ok(Value::Null));
            for value in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
                assert_eq!(function.apply(&[number(value)]), Err(EvalError::NonFinite));
            }
            assert_eq!(
                function.apply(&[Value::Text("1".into())]),
                Err(EvalError::TypeMismatch)
            );
            assert_eq!(function.apply(&[]), Err(EvalError::UnsupportedExpression));
            assert_eq!(
                function.apply(&[number(1.0), number(1.0)]),
                Err(EvalError::UnsupportedExpression)
            );
        }
        for (function, value) in [
            (MathFunction::Sqr, -1.0),
            (MathFunction::Log, -1.0),
            (MathFunction::Log, 0.0),
            (MathFunction::Log, -0.0),
        ] {
            assert_eq!(function.apply(&[number(value)]), Err(EvalError::Domain));
        }
        assert_eq!(
            MathFunction::Exp.apply(&[number(710.0)]),
            Err(EvalError::Overflow)
        );
        assert_eq!(result(MathFunction::Exp, -1000.0), 0.0);
        assert!(result(MathFunction::Exp, 709.0).is_finite());
        assert_eq!(
            result(MathFunction::Sqr, -0.0).to_bits(),
            (-0.0_f64).to_bits()
        );
        assert!(result(MathFunction::Log, f64::from_bits(1)).is_finite());
    }
    proptest! {
        #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
        #[test]
        fn generated_finite_angles_preserve_trigonometric_identities(angle in -10000_f64..10000_f64) {
            let sin=result(MathFunction::Sin,angle);let cos=result(MathFunction::Cos,angle);
            prop_assert!((sin*sin+cos*cos-1.0).abs()<1e-14);
            prop_assert_eq!(result(MathFunction::Sin,-angle),-sin);
            prop_assert_eq!(result(MathFunction::Cos,-angle),cos);
            prop_assert_eq!(result(MathFunction::Atn,-angle),-result(MathFunction::Atn,angle));
        }
    }
}
