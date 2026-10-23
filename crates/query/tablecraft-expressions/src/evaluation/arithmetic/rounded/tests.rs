use super::*;
use crate::{
    evaluation::{EvalLimits, NoBindings},
    functions::conversions::text::NumberFormat,
    numeric::OperandDeclaration,
};
use std::cell::Cell;
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Decimal, Double, LegacyDateTime, Single},
};

fn declarations(variant: bool) -> ArithmeticDeclarations {
    ArithmeticDeclarations {
        left: if variant {
            OperandDeclaration::Variant
        } else {
            OperandDeclaration::Scalar
        },
        right: OperandDeclaration::Scalar,
    }
}

#[test]
fn original_declarations_control_overflow_after_checked_operand_rounding() {
    for (left, right, widened) in [
        (
            Value::Single(Single::from_float(-2147483648.0)),
            Value::Int16(-1),
            Value::Double(Double::from_float(2147483648.0)),
        ),
        (
            Value::Decimal(Decimal::new(-2147483648, 0).unwrap()),
            Value::Int16(-1),
            Value::Double(Double::from_float(2147483648.0)),
        ),
        (
            Value::Int16(-32768),
            Value::Double(Double::from_float(-1.0)),
            Value::Int32(32768),
        ),
        (
            Value::Double(Double::from_float(-2147483648.0)),
            Value::Int16(-1),
            Value::Double(Double::from_float(2147483648.0)),
        ),
        (
            Value::Currency(Currency::from_scaled(-21474836480000)),
            Value::Int16(-1),
            Value::Double(Double::from_float(2147483648.0)),
        ),
        (
            Value::Int16(-32768),
            Value::DateTime(LegacyDateTime::from_ole_days(-1.0).unwrap()),
            Value::Int32(32768),
        ),
        (
            Value::Text("-2147483648".into()),
            Value::Int16(-1),
            Value::Double(Double::from_float(2147483648.0)),
        ),
    ] {
        for variant in [false, true] {
            let mut budget = ResourceBudget::new(1000, 0, 1000);
            let result = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
                .with_number_format(NumberFormat::EnUs)
                .apply_declared_rounded_arithmetic(
                    Arithmetic::IntegerDivide,
                    &left,
                    &right,
                    declarations(variant),
                );
            assert_eq!(
                result,
                if variant {
                    Ok(widened.clone())
                } else {
                    Err(EvalError::Overflow)
                }
            );
        }
    }
}

#[test]
fn lexical_rounding_null_ranges_and_zero_divisors_retain_contextual_rules() {
    let mut budget = ResourceBudget::new(10000, 0, 10000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
        .with_number_format(NumberFormat::EnUs);
    let divisor = Value::Text("2.50000000000000000000000000000000001".into());
    assert_eq!(
        evaluator.apply_declared_rounded_arithmetic(
            Arithmetic::IntegerDivide,
            &Value::Int16(7),
            &divisor,
            declarations(false)
        ),
        Ok(Value::Int16(2))
    );
    assert_eq!(
        evaluator.apply_declared_rounded_arithmetic(
            Arithmetic::Modulo,
            &Value::Int16(7),
            &divisor,
            declarations(false)
        ),
        Ok(Value::Int16(1))
    );
    for variant in [false, true] {
        assert_eq!(
            evaluator.apply_declared_rounded_arithmetic(
                Arithmetic::IntegerDivide,
                &Value::Int16(7),
                &Value::Double(Double::from_float(0.49)),
                declarations(variant)
            ),
            Err(EvalError::DivisionByZero)
        );
        assert_eq!(
            evaluator.apply_declared_rounded_arithmetic(
                Arithmetic::IntegerDivide,
                &Value::Int16(7),
                &Value::Double(Double::from_float(32768.0)),
                declarations(variant)
            ),
            Err(EvalError::Overflow)
        );
    }
    assert_eq!(
        evaluator.apply_declared_rounded_arithmetic(
            Arithmetic::IntegerDivide,
            &Value::Null,
            &Value::Text("bad".into()),
            declarations(false)
        ),
        Ok(Value::Null)
    );
    assert_eq!(
        evaluator.apply_declared_rounded_arithmetic(
            Arithmetic::Add,
            &Value::Null,
            &Value::Int16(1),
            declarations(false)
        ),
        Err(EvalError::UnsupportedExpression)
    );
    assert_eq!(
        evaluator.apply_declared_rounded_arithmetic(
            Arithmetic::Modulo,
            &Value::Binary(vec![1]),
            &Value::Int16(1),
            declarations(false)
        ),
        Err(EvalError::TypeMismatch)
    );
}

#[test]
fn rounded_declaration_conversions_share_work_bytes_and_cancellation() {
    let calls = Cell::new(0);
    let invoke = |stop, bytes, steps| {
        calls.set(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        let mut budget = ResourceBudget::new(bytes, 0, steps);
        Evaluator::new(&NoBindings, &mut budget, EvalLimits::default())
            .with_number_format(NumberFormat::EnUs)
            .with_cancellation(&cancelled)
            .apply_declared_rounded_arithmetic(
                Arithmetic::IntegerDivide,
                &Value::Text("9.5".into()),
                &Value::Text("2.5".into()),
                declarations(false),
            )
    };
    assert_eq!(invoke(usize::MAX, 1000, 1000), Ok(Value::Int32(5)));
    let observed = calls.get();
    for stop in 1..=observed {
        assert_eq!(invoke(stop, 1000, 1000), Err(EvalError::Cancelled));
    }
    assert!(invoke(usize::MAX, 0, 1000).is_err());
    assert!(invoke(usize::MAX, 1000, 0).is_err());
}
