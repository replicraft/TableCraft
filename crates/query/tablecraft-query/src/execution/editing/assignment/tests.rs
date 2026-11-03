use super::*;
use tablecraft_core::values::Double;

fn target(kind: ScalarType, nullable: bool) -> Column {
    Column::new("Target".to_owned(), kind, nullable).unwrap()
}

#[test]
fn exact_integral_assignments_and_scale_do_not_change_other_carrier_rules() {
    for value in [
        Value::Byte(7),
        Value::Int16(7),
        Value::Int32(7),
        Value::Int64(7),
    ] {
        for (kind, expected) in [
            (ScalarType::Int64, Value::Int64(7)),
            (
                ScalarType::Currency,
                Value::Currency(Currency::from_scaled(70_000)),
            ),
        ] {
            assert_eq!(
                stored_value(
                    value.clone(),
                    &target(kind, false),
                    &mut ResourceBudget::new(0, 0, 1),
                    &|| false
                ),
                Ok(expected),
            );
        }
    }

    for kind in [ScalarType::Int64, ScalarType::Currency] {
        for value in [
            Value::Boolean(false),
            Value::Double(Double::from_float(7.0)),
            Value::Text("7".to_owned()),
        ] {
            assert_eq!(
                stored_value(
                    value,
                    &target(kind, true),
                    &mut ResourceBudget::new(0, 0, 0),
                    &|| false
                ),
                Err(QueryError::Unsupported),
            );
        }
        assert_eq!(
            stored_value(
                Value::Null,
                &target(kind, true),
                &mut ResourceBudget::new(0, 0, 0),
                &|| true
            ),
            Ok(Value::Null),
        );
        assert_eq!(
            stored_value(
                Value::Null,
                &target(kind, false),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            ),
            Err(QueryError::Unsupported),
        );
    }
}

#[test]
fn currency_integer_bounds_are_checked_without_rounding_or_saturation() {
    for integer in [-922_337_203_685_477_i64, 922_337_203_685_477] {
        assert_eq!(
            stored_value(
                Value::Int64(integer),
                &target(ScalarType::Currency, false),
                &mut ResourceBudget::new(0, 0, 1),
                &|| false
            ),
            Ok(Value::Currency(Currency::from_scaled(integer * 10_000))),
        );
    }
    for integer in [
        i64::MIN,
        -922_337_203_685_478,
        922_337_203_685_478,
        i64::MAX,
    ] {
        assert_eq!(
            stored_value(
                Value::Int64(integer),
                &target(ScalarType::Currency, false),
                &mut ResourceBudget::new(0, 0, 1),
                &|| false
            ),
            Err(EvalError::Overflow.into()),
        );
    }
}

#[test]
fn only_new_conversions_consume_the_extra_work_and_check_cancellation() {
    let column = target(ScalarType::Currency, false);
    assert!(matches!(
        stored_value(
            Value::Int16(0),
            &column,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(QueryError::Limit(_)),
    ));
    assert_eq!(
        stored_value(
            Value::Int16(0),
            &column,
            &mut ResourceBudget::new(0, 0, 1),
            &|| true
        ),
        Err(QueryError::Cancelled),
    );
    let exact = Value::Currency(Currency::from_scaled(i64::MIN));
    assert_eq!(
        stored_value(
            exact.clone(),
            &column,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Ok(exact),
    );
    assert_eq!(
        update_value(
            Value::Int16(7),
            &target(ScalarType::Int32, false),
            UpdateProfile::ExistingInt32,
            &mut ResourceBudget::new(0, 0, 0),
            &|| true
        ),
        Ok(Value::Int32(7)),
    );
}
