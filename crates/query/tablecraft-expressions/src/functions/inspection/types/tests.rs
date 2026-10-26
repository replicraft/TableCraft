use super::*;
use crate::arrays::{ArrayValue, ArrayView};
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single,
};

fn scalar(function: ValueInspectionFunction, value: &Value) -> InspectionResult {
    function.apply(InspectionInput::Scalar(value)).unwrap()
}

#[test]
fn materialized_carriers_use_independent_vba_names_and_subtype_goldens() {
    for (value, name, code) in [
        (Value::Null, "Null", 1),
        (Value::Int16(-1), "Integer", 2),
        (Value::Int32(-1), "Long", 3),
        (Value::Single(Single::from_float(1.)), "Single", 4),
        (Value::Double(Double::from_float(f64::NAN)), "Double", 5),
        (Value::Currency(Currency::from_scaled(10)), "Currency", 6),
        (
            Value::DateTime(LegacyDateTime::from_ole_days(0.).unwrap()),
            "Date",
            7,
        ),
        (Value::Text("secret".into()), "String", 8),
        (Value::Boolean(true), "Boolean", 11),
        (Value::Decimal(Decimal::new(0, 3).unwrap()), "Decimal", 14),
        (Value::Byte(255), "Byte", 17),
        (Value::Int64(i64::MIN), "LongLong", 20),
        (Value::Binary(vec![0xff, 0]), "Byte()", 8209),
    ] {
        assert_eq!(
            scalar(ValueInspectionFunction::TypeName, &value),
            InspectionResult::TypeName(name)
        );
        assert_eq!(
            scalar(ValueInspectionFunction::VarType, &value),
            InspectionResult::Scalar(Value::Int16(code))
        );
        assert_eq!(
            scalar(ValueInspectionFunction::IsArray, &value),
            InspectionResult::Scalar(Value::Boolean(code == 8209))
        );
        assert_eq!(
            scalar(ValueInspectionFunction::IsEmpty, &value),
            InspectionResult::Scalar(Value::Boolean(false))
        );
        assert_eq!(
            scalar(ValueInspectionFunction::IsError, &value),
            InspectionResult::Scalar(Value::Boolean(false))
        );
    }
}

#[test]
fn explicit_states_do_not_collapse_into_null_or_catch_operational_failures() {
    for (value, name, code, empty, error) in [
        (ExpressionValue::Empty, "Empty", 0, true, false),
        (ExpressionValue::Error(123456789), "Error", 10, false, true),
        (ExpressionValue::Value(Value::Null), "Null", 1, false, false),
    ] {
        let input = InspectionInput::Expression(&value);
        assert_eq!(
            ValueInspectionFunction::TypeName.apply(input),
            Ok(InspectionResult::TypeName(name))
        );
        assert_eq!(
            ValueInspectionFunction::VarType.apply(input),
            Ok(InspectionResult::Scalar(Value::Int16(code)))
        );
        assert_eq!(
            ValueInspectionFunction::IsEmpty.apply(input),
            Ok(InspectionResult::Scalar(Value::Boolean(empty)))
        );
        assert_eq!(
            ValueInspectionFunction::IsError.apply(input),
            Ok(InspectionResult::Scalar(Value::Boolean(error)))
        );
    }

    for function in [
        ValueInspectionFunction::TypeName,
        ValueInspectionFunction::VarType,
        ValueInspectionFunction::IsEmpty,
        ValueInspectionFunction::IsError,
        ValueInspectionFunction::IsArray,
    ] {
        assert_eq!(
            function.apply(InspectionInput::Expression(&ExpressionValue::Missing)),
            Err(EvalError::UnsupportedExpression)
        );
    }

    let wrapped = ExpressionValue::Value(Value::Binary(vec![]));
    assert_eq!(
        ValueInspectionFunction::IsArray.apply(InspectionInput::Expression(&wrapped)),
        Ok(InspectionResult::Scalar(Value::Boolean(true)))
    );
}

#[test]
fn variant_array_metadata_is_declared_and_never_inferred_from_elements() {
    for elements in [
        vec![],
        vec![Value::Int16(1)],
        vec![Value::Null, Value::Text("secret".into())],
    ] {
        let array = ArrayValue::from_owned(elements, 0).unwrap();
        let input = InspectionInput::Array(ArrayView::Variants(&array));
        assert_eq!(
            ValueInspectionFunction::TypeName.apply(input),
            Ok(InspectionResult::TypeName("Variant()"))
        );
        assert_eq!(
            ValueInspectionFunction::VarType.apply(input),
            Ok(InspectionResult::Scalar(Value::Int16(8204)))
        );
        assert_eq!(
            ValueInspectionFunction::IsArray.apply(input),
            Ok(InspectionResult::Scalar(Value::Boolean(true)))
        );
    }
}

#[test]
fn extended_dates_keep_their_unqualified_language_type_boundary() {
    let value = Value::DateTimeExtended(ExtendedDateTime::new(2026, 10, 21, 0, 0, 0, 0).unwrap());
    for function in [
        ValueInspectionFunction::TypeName,
        ValueInspectionFunction::VarType,
    ] {
        assert_eq!(
            function.apply(InspectionInput::Scalar(&value)),
            Err(EvalError::TypeMismatch)
        );
    }
    assert_eq!(
        scalar(ValueInspectionFunction::IsArray, &value),
        InspectionResult::Scalar(Value::Boolean(false))
    );
}

#[test]
fn stored_guids_share_byte_array_inspection_without_converting_the_payload() {
    for canonical in [[0; 16], [0xff; 16], [0x12; 16]] {
        let value = Value::Guid(Guid::from_bytes(canonical));
        let expression = ExpressionValue::Value(value.clone());
        for input in [
            InspectionInput::Scalar(&value),
            InspectionInput::Expression(&expression),
        ] {
            assert_eq!(
                ValueInspectionFunction::TypeName.apply(input),
                Ok(InspectionResult::TypeName("Byte()"))
            );
            for (function, expected) in [
                (ValueInspectionFunction::VarType, Value::Int16(8209)),
                (ValueInspectionFunction::IsArray, Value::Boolean(true)),
                (ValueInspectionFunction::IsEmpty, Value::Boolean(false)),
                (ValueInspectionFunction::IsError, Value::Boolean(false)),
            ] {
                assert_eq!(
                    function.apply(input),
                    Ok(InspectionResult::Scalar(expected))
                );
            }
        }
    }
}
