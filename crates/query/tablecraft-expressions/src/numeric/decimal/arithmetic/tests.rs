use super::*;
use Arithmetic::*;
use proptest::prelude::*;
use tablecraft_core::values::{Currency, Decimal, Double, Single};
fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}
fn calculate(op: Arithmetic, left: Value, right: Value) -> Result<Value, EvalError> {
    numeric::arithmetic(op, &left, &right)
}
fn assert_numeric(result: Result<Value, EvalError>, expected: Value) {
    let actual = result.unwrap();
    assert!(matches!(actual, Value::Decimal(_)));
    assert_eq!(
        crate::numeric::compare(&actual, &expected),
        Ok(std::cmp::Ordering::Equal)
    );
}
#[test]
fn decimal_dominates_exact_carriers_without_float_intermediates() {
    for other in [
        Value::Boolean(true),
        Value::Int16(-1),
        Value::Int32(-1),
        Value::Int64(-1),
        decimal(-1, 0),
    ] {
        assert_eq!(
            calculate(Add, decimal(125, 2), other.clone()),
            Ok(decimal(25, 2))
        );
        assert_eq!(
            calculate(Subtract, other, decimal(125, 2)),
            Ok(decimal(-225, 2))
        );
    }
    assert_eq!(
        calculate(Add, decimal(1, 0), Value::Int64(9007199254740993)),
        Ok(decimal(9007199254740994, 0))
    );
    assert_eq!(
        calculate(
            Add,
            decimal(1, 0),
            Value::Currency(Currency::from_scaled(i64::MAX))
        ),
        Ok(decimal(i128::from(i64::MAX) + 10000, 4))
    );
    assert_eq!(
        calculate(Multiply, decimal(125, 2), Value::Byte(2)),
        Ok(decimal(250, 2))
    );
    assert_eq!(
        calculate(Multiply, decimal(125, 2), decimal(20, 1)),
        Ok(decimal(2500, 3))
    );
    assert_eq!(
        calculate(Divide, decimal(1, 0), Value::Int64(8)),
        Ok(decimal(125, 3))
    );
    assert_numeric(
        calculate(Divide, Value::Int64(1), decimal(2, 0)),
        decimal(5, 1),
    );
}
#[test]
fn bounded_rounding_nonterminating_division_and_overflow_have_goldens() {
    assert_eq!(
        calculate(Divide, decimal(1, 0), decimal(3, 0)),
        Ok(decimal(3333333333333333333333333333, 28))
    );
    assert_eq!(
        calculate(Divide, decimal(2, 0), decimal(3, 0)),
        Ok(decimal(6666666666666666666666666667, 28))
    );
    assert_eq!(
        calculate(Multiply, decimal(1, 28), decimal(5, 1)),
        Ok(decimal(0, 28))
    );
    assert_eq!(
        calculate(Multiply, decimal(3, 28), decimal(5, 1)),
        Ok(decimal(2, 28))
    );
    assert_eq!(
        calculate(Add, decimal(Decimal::MAX_COEFFICIENT, 0), decimal(4, 1)),
        Ok(decimal(Decimal::MAX_COEFFICIENT, 0))
    );
    assert_eq!(
        calculate(
            Subtract,
            decimal(Decimal::MAX_COEFFICIENT, 0),
            decimal(5, 1)
        ),
        Ok(decimal(Decimal::MAX_COEFFICIENT - 1, 0))
    );
    for op in [Add, Multiply] {
        assert_eq!(
            calculate(op, decimal(Decimal::MAX_COEFFICIENT, 0), decimal(2, 0)),
            Err(EvalError::Overflow)
        );
    }
    assert_eq!(
        calculate(Divide, decimal(Decimal::MAX_COEFFICIENT, 0), decimal(1, 1)),
        Err(EvalError::Overflow)
    );
    assert_numeric(
        calculate(Divide, decimal(1, 28), decimal(2, 0)),
        decimal(0, 28),
    );
}
#[test]
fn normalized_floats_power_null_errors_and_integer_division_stay_explicit() {
    for other in [
        Value::Single(Single::from_float(0.1)),
        Value::Double(Double::from_float(0.1)),
    ] {
        assert_eq!(calculate(Add, decimal(1, 1), other), Ok(decimal(2, 1)));
    }
    assert_eq!(
        calculate(Power, decimal(2, 0), decimal(3, 0)),
        Ok(Value::Double(Double::from_float(8.0)))
    );
    assert_eq!(
        calculate(Power, decimal(-2, 0), decimal(5, 1)),
        Err(EvalError::Domain)
    );
    assert_eq!(
        calculate(Divide, decimal(1, 0), decimal(0, 28)),
        Err(EvalError::DivisionByZero)
    );
    assert_eq!(
        calculate(Divide, Value::Null, decimal(0, 28)),
        Ok(Value::Null)
    );
    assert_eq!(
        calculate(
            Add,
            decimal(1, 0),
            Value::Double(Double::from_float(f64::INFINITY))
        ),
        Err(EvalError::NonFinite)
    );
    assert_eq!(
        calculate(Subtract, decimal(1, 0), Value::Text("1".into())),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        calculate(IntegerDivide, decimal(95, 1), decimal(25, 1)),
        Ok(Value::Int64(5))
    );
    assert_eq!(
        calculate(Modulo, decimal(-126, 1), decimal(5, 0)),
        Ok(Value::Int64(-3))
    );
}
proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn bounded_exact_inputs_match_independent_i128_arithmetic(a in any::<i32>(),b in any::<i32>(),sa in 0_u8..=6,sb in 0_u8..=6) {
        let scale=sa.max(sb);
        let aligned_a=i128::from(a)*10_i128.pow(u32::from(scale-sa));
        let aligned_b=i128::from(b)*10_i128.pow(u32::from(scale-sb));
        for (op,coefficient,result_scale) in [(Add,aligned_a+aligned_b,scale),(Subtract,aligned_a-aligned_b,scale),(Multiply,i128::from(a)*i128::from(b),sa+sb)] {
            let actual=calculate(op,decimal(i128::from(a),sa),decimal(i128::from(b),sb)).unwrap();
            let Value::Decimal(actual)=actual else { unreachable!() };
            // Compare exact rational results without assuming the library's zero scale.
            prop_assert_eq!(actual.coefficient()*10_i128.pow(u32::from(result_scale)),coefficient*10_i128.pow(u32::from(actual.scale())));
        }
    }
}

proptest! {
    #![proptest_config(crate::test_support::config())]
    #[test]
    fn bounded_division_matches_independent_rational_half_even_rounding(a in -1_000_000_i64..=1_000_000,b in 1_i64..=1_000_000) {
        let actual=calculate(Divide,decimal(i128::from(a),0),decimal(i128::from(b),0)).unwrap();
        let expected=(0_u8..=28).rev().find_map(|scale| {
            let numerator=i128::from(a).abs()*10_i128.pow(u32::from(scale));
            let denominator=i128::from(b);
            let whole=numerator/denominator;
            let twice=(numerator%denominator)*2;
            let rounded=whole+i128::from(twice>denominator || (twice==denominator && whole%2!=0));
            (rounded<=Decimal::MAX_COEFFICIENT).then(|| decimal(rounded*i128::from(a.signum()),scale))
        }).unwrap();
        prop_assert_eq!(crate::numeric::compare(&actual,&expected),Ok(std::cmp::Ordering::Equal));
    }
}
