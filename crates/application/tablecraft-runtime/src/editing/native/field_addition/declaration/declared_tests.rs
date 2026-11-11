//! A gallery-equivalent scalar cannot be relabeled as an unadorned SQL request.
use super::*;
use tablecraft_core::values::{Currency, Value};

#[test]
fn actual_absent_currency_policies_produce_sql_origin() {
    let native = CreateColumn::new("Amount", ScalarType::Currency);
    assert_eq!(
        captured_unadorned(native).unwrap(),
        FieldAdditionOrigin::SqlUnadorned(
            TypedFieldAdditionDeclaration::fixed(ScalarType::Currency).unwrap()
        )
    );
}

#[test]
fn explicit_policies_are_not_silently_erased() {
    let zero = Value::Currency(Currency::from_scaled(0));
    for native in [
        CreateColumn::new("Amount", ScalarType::Currency).with_required(false),
        CreateColumn::new("Amount", ScalarType::Currency)
            .with_default_literal(&zero)
            .unwrap(),
        CreateColumn::new("Amount", ScalarType::Int32).with_required(false),
        CreateColumn::new("Amount", ScalarType::Guid).with_required(false),
    ] {
        assert!(matches!(
            captured_unadorned(native),
            Err(Error::SchemaSqlUnsupported)
        ));
    }
}

#[test]
fn actual_native_text_units_and_decimal_parameters_survive_as_unadorned_shape_data() {
    let text = CreateColumn::short_text("Label", 80).unwrap();
    assert_eq!(
        captured_unadorned(text).unwrap(),
        FieldAdditionOrigin::SqlUnadorned(TypedFieldAdditionDeclaration::short_text(
            NonZeroU8::new(80).unwrap(),
            false
        ))
    );
    let decimal = CreateColumn::decimal(
        "Amount",
        tablecraft_schema::DecimalConstraints::new(28, 4).unwrap(),
    );
    assert_eq!(
        captured_unadorned(decimal).unwrap(),
        FieldAdditionOrigin::SqlUnadorned(TypedFieldAdditionDeclaration::decimal(28, 4).unwrap())
    );
    assert!(captured_unadorned(text.with_allow_zero_length(true).unwrap()).is_err());
}
