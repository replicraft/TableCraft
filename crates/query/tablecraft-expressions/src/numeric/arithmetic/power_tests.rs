use super::*;

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

#[test]
fn converted_zero_negative_powers_are_domain_errors_without_changing_division() {
    for zero in [0.0, -0.0] {
        for exponent in [-1.0, -0.5, -f64::from_bits(1), -f64::MAX] {
            assert_eq!(
                arithmetic(Arithmetic::Power, &double(zero), &double(exponent)),
                Err(EvalError::Domain)
            );
        }

        assert_eq!(
            arithmetic(Arithmetic::Power, &double(zero), &double(0.0)),
            Ok(double(1.0))
        );
        assert_eq!(
            arithmetic(Arithmetic::Divide, &double(zero), &double(0.0)),
            Err(EvalError::Overflow)
        );
    }

    assert_eq!(
        arithmetic(Arithmetic::Divide, &double(1.0), &double(0.0)),
        Err(EvalError::DivisionByZero)
    );
}

#[test]
fn power_preserves_signed_underflow_and_finite_domain_boundaries() {
    for (base, exponent, expected) in [
        (-0.0, 3.0, -0.0_f64),
        (-0.0, 2.0, 0.0),
        (-0.5, 1075.0, -0.0),
        (0.5, 1075.0, 0.0),
        (-2.0, -3.0, -0.125),
        (-2.0, 4.0, 16.0),
        (4.0, 0.5, 2.0),
    ] {
        let Value::Double(result) =
            arithmetic(Arithmetic::Power, &double(base), &double(exponent)).unwrap()
        else {
            panic!("Power must produce Double");
        };
        assert_eq!(result.to_float().to_bits(), expected.to_bits());
    }

    assert_eq!(
        arithmetic(Arithmetic::Power, &double(-2.0), &double(0.5)),
        Err(EvalError::Domain)
    );
    assert_eq!(
        arithmetic(Arithmetic::Power, &double(2.0), &double(1024.0)),
        Err(EvalError::Overflow)
    );
}

#[test]
fn power_admission_still_precedes_null_propagation() {
    assert_eq!(
        arithmetic(Arithmetic::Power, &Value::Null, &double(-1.0)),
        Ok(Value::Null)
    );
    assert_eq!(
        arithmetic(Arithmetic::Power, &double(f64::NAN), &Value::Null),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        arithmetic(Arithmetic::Power, &Value::Binary(vec![]), &Value::Null),
        Err(EvalError::TypeMismatch)
    );
}
