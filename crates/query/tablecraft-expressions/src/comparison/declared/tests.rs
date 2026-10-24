use super::*;
use tablecraft_core::values::{
    Currency, Decimal, Double, ExpressionString, LegacyDateTime, Single,
};

#[test]
fn lossless_variant_strings_keep_category_order_without_decoding() {
    let raw = ExpressionValue::StringBytes(ExpressionString::from_owned(vec![0, 216, 65]));
    let number = ExpressionValue::Value(Value::Int32(0));
    assert_eq!(
        variant_expression_text_numeric(
            Comparison::Greater,
            &raw,
            &number,
            OperandDeclaration::Variant,
            OperandDeclaration::Variant,
        ),
        Ok(Some(Truth::True))
    );
    assert_eq!(
        variant_expression_text_numeric(
            Comparison::Less,
            &number,
            &raw,
            OperandDeclaration::Variant,
            OperandDeclaration::Variant,
        ),
        Ok(Some(Truth::True))
    );
    assert_eq!(
        variant_expression_text_numeric(
            Comparison::Greater,
            &raw,
            &number,
            OperandDeclaration::Scalar,
            OperandDeclaration::Variant,
        ),
        Ok(None)
    );
    let nonfinite = ExpressionValue::Value(Value::Double(Double::from_float(f64::INFINITY)));
    assert_eq!(
        variant_expression_text_numeric(
            Comparison::Equal,
            &raw,
            &nonfinite,
            OperandDeclaration::Variant,
            OperandDeclaration::Variant,
        ),
        Err(EvalError::NonFinite)
    );
}

#[test]
fn all_finite_numeric_types_precede_strings_in_both_directions() {
    let numbers = [
        Value::Byte(u8::MAX),
        Value::Int16(i16::MIN),
        Value::Int32(i32::MAX),
        Value::Int64(i64::MIN),
        Value::Single(Single::from_float(-0.0)),
        Value::Double(Double::from_float(f64::MAX)),
        Value::Currency(Currency::from_scaled(i64::MAX)),
        Value::Decimal(Decimal::new(1, 28).unwrap()),
    ];
    for number in numbers {
        for text in ["", "0", "not numeric", "9,999", "∞"] {
            let text = Value::Text(text.into());
            for (operator, forward, reverse) in [
                (Comparison::Equal, false, false),
                (Comparison::NotEqual, true, true),
                (Comparison::Less, true, false),
                (Comparison::LessOrEqual, true, false),
                (Comparison::Greater, false, true),
                (Comparison::GreaterOrEqual, false, true),
            ] {
                for (left, right, expected) in
                    [(&number, &text, forward), (&text, &number, reverse)]
                {
                    let result = variant_text_numeric(
                        operator,
                        left,
                        right,
                        OperandDeclaration::Variant,
                        OperandDeclaration::Variant,
                    )
                    .unwrap();
                    assert_eq!(
                        result,
                        Some(if expected { Truth::True } else { Truth::False })
                    );
                }
            }
        }
    }
}

#[test]
fn declarations_and_non_numeric_kinds_do_not_invent_a_category_order() {
    let text = Value::Text("1".into());
    let numeric = Value::Int16(1);
    for (left, right) in [
        (OperandDeclaration::Scalar, OperandDeclaration::Scalar),
        (OperandDeclaration::Scalar, OperandDeclaration::Variant),
        (OperandDeclaration::Variant, OperandDeclaration::Scalar),
    ] {
        assert_eq!(
            variant_text_numeric(Comparison::Equal, &text, &numeric, left, right).unwrap(),
            None
        );
    }
    for value in [
        Value::Null,
        Value::Boolean(true),
        Value::DateTime(LegacyDateTime::from_ole_days(1.0).unwrap()),
        Value::Text("2".into()),
        Value::Binary(vec![1]),
    ] {
        for (left, right) in [(&text, &value), (&value, &text)] {
            assert_eq!(
                variant_text_numeric(
                    Comparison::Equal,
                    left,
                    right,
                    OperandDeclaration::Variant,
                    OperandDeclaration::Variant
                )
                .unwrap(),
                None
            );
        }
    }
}

#[test]
fn category_ordering_does_not_hide_nonfinite_values() {
    let text = Value::Text(String::new());
    for number in [
        Value::Single(Single::from_float(f32::NAN)),
        Value::Single(Single::from_float(f32::INFINITY)),
        Value::Double(Double::from_float(f64::NAN)),
        Value::Double(Double::from_float(f64::NEG_INFINITY)),
    ] {
        for (left, right) in [(&text, &number), (&number, &text)] {
            assert_eq!(
                variant_text_numeric(
                    Comparison::Less,
                    left,
                    right,
                    OperandDeclaration::Variant,
                    OperandDeclaration::Variant
                ),
                Err(EvalError::NonFinite)
            );
        }
    }
}
