use super::*;
use crate::EvalError;
use proptest::prelude::*;
use tablecraft_core::values::{Double, Value};

#[test]
fn bounds_retain_explicit_bases_empty_extents_and_checked_long_arithmetic() {
    for (lower, values, upper) in [
        (0, vec![], -1),
        (1, vec![Value::Null, Value::Int16(0)], 2),
        (-3, vec![Value::Int32(1)], -3),
        (i32::MAX, vec![Value::Null], i32::MAX),
    ] {
        let array = ArrayValue::from_owned(values, lower).unwrap();
        let view = ArrayView::Variants(&array);
        assert_eq!(
            BoundFunction::LBound.apply(view, None),
            Ok(Value::Int32(lower))
        );
        assert_eq!(
            BoundFunction::UBound.apply(view, Some(&Value::Int16(1))),
            Ok(Value::Int32(upper))
        );
    }

    assert_eq!(
        ArrayValue::from_owned(vec![], i32::MIN).unwrap_err(),
        EvalError::ArrayBounds
    );
    assert_eq!(
        ArrayValue::from_owned(vec![Value::Null; 2], i32::MAX).unwrap_err(),
        EvalError::ArrayBounds
    );
    assert_eq!(
        ArrayValue::from_owned(vec![Value::Null; 65_537], 0).unwrap_err(),
        EvalError::ArrayLimit
    );
    assert_eq!(
        ArrayValue::from_owned(vec![Value::Null; 65_536], 0)
            .unwrap()
            .upper_bound(),
        65_535
    );

    let array = ArrayValue::from_owned(vec![Value::Text("secret".into())], -1).unwrap();
    assert_eq!(
        format!("{array:?}"),
        "ArrayValue { len: 1, lower: -1, upper: -1, .. }"
    );
}

#[test]
fn dimensions_use_checked_long_conversion_and_binary_bounds_do_not_decode_payloads() {
    let view = ArrayView::Bytes(&[0xff, 0, 0xc0]);
    assert_eq!(BoundFunction::LBound.apply(view, None), Ok(Value::Int32(0)));
    assert_eq!(BoundFunction::UBound.apply(view, None), Ok(Value::Int32(2)));
    assert_eq!(
        BoundFunction::UBound.apply(ArrayView::Bytes(&[]), None),
        Ok(Value::Int32(-1))
    );

    for dimension in [
        Value::Int16(0),
        Value::Int16(-1),
        Value::Int32(2),
        Value::Boolean(true),
    ] {
        assert_eq!(
            BoundFunction::UBound.apply(view, Some(&dimension)),
            Err(EvalError::ArrayBounds)
        );
    }
    assert_eq!(
        BoundFunction::LBound.apply(view, Some(&Value::Null)),
        Err(EvalError::InvalidNullConversion)
    );
    assert_eq!(
        BoundFunction::LBound.apply(view, Some(&Value::Text("1".into()))),
        Err(EvalError::TypeMismatch)
    );
    assert_eq!(
        BoundFunction::UBound.apply(view, Some(&Value::Double(Double::from_float(1.4)))),
        Ok(Value::Int32(2))
    );
    assert_eq!(
        BoundFunction::UBound.apply(view, Some(&Value::Double(Double::from_float(1.5)))),
        Err(EvalError::ArrayBounds)
    );
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x424f554e4453),
        ..ProptestConfig::default()
    })]

    #[test]
    fn owned_bounds_match_independent_wide_extent_arithmetic(
        lower in any::<i32>(),
        length in 0_usize..128,
    ) {
        let expected = i64::from(lower) + length as i64 - 1;
        let result = ArrayValue::from_owned(vec![Value::Null; length], lower);

        if let Ok(expected) = i32::try_from(expected) {
            let array = result.unwrap();
            prop_assert_eq!(array.lower_bound(), lower);
            prop_assert_eq!(array.upper_bound(), expected);
            prop_assert_eq!(array.elements().len(), length);
        } else {
            prop_assert_eq!(result.unwrap_err(), EvalError::ArrayBounds);
        }
    }
}
