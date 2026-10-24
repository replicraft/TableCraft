use super::*;
use crate::evaluation::{EvalLimits, NoBindings};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, ExtendedDateTime, Guid},
};
use tablecraft_sql::select::BinaryOperator;

#[test]
fn contextual_and_declared_rounded_apis_validate_carriers_before_null_results() {
    for value in [
        Value::Binary(vec![1]),
        Value::Guid(Guid::from_bytes([1; 16])),
        Value::DateTimeExtended(ExtendedDateTime::new(1, 1, 1, 0, 0, 0, 0).unwrap()),
        Value::Double(Double::from_float(f64::INFINITY)),
    ] {
        let expected = if matches!(value, Value::Double(_)) {
            EvalError::NonFinite
        } else {
            EvalError::TypeMismatch
        };
        let mut budget = ResourceBudget::new(0, 0, 1000);
        let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
        for (left, right) in [(&Value::Null, &value), (&value, &Value::Null)] {
            for operator in [
                BinaryOperator::Add,
                BinaryOperator::Multiply,
                BinaryOperator::Modulo,
            ] {
                assert_eq!(evaluator.apply_binary(operator, left, right), Err(expected));
            }
            for operator in [Arithmetic::IntegerDivide, Arithmetic::Modulo] {
                assert_eq!(
                    evaluator.apply_declared_rounded_arithmetic(
                        operator,
                        left,
                        right,
                        ArithmeticDeclarations::default()
                    ),
                    Err(expected)
                );
            }
        }
    }
}

#[test]
fn admitted_null_and_text_operands_do_not_require_parsing_policies_or_converted_ranges() {
    let text = Value::Text("not a numeric spelling".into());
    let large = Value::Double(Double::from_float(f64::MAX));
    let mut budget = ResourceBudget::new(0, 0, 1000);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    for value in [&text, &large] {
        for (left, right) in [(&Value::Null, value), (value, &Value::Null)] {
            for operator in [
                BinaryOperator::Add,
                BinaryOperator::IntegerDivide,
                BinaryOperator::Power,
            ] {
                assert_eq!(
                    evaluator.apply_binary(operator, left, right),
                    Ok(Value::Null)
                );
            }
            assert_eq!(
                evaluator.apply_declared_rounded_arithmetic(
                    Arithmetic::Modulo,
                    left,
                    right,
                    ArithmeticDeclarations::default()
                ),
                Ok(Value::Null)
            );
        }
    }
}
