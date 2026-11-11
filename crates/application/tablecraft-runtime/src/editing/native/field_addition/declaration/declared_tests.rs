//! A gallery-equivalent scalar cannot be relabeled as an unadorned SQL request.
use super::*;
use tablecraft_core::values::{Currency, Value};

#[test]
fn actual_absent_currency_policies_produce_sql_origin() {
    let native = CreateColumn::new("Amount", ScalarType::Currency);
    assert_eq!(
        captured_declared(native).unwrap(),
        FieldAdditionOrigin::SqlDeclared {
            scalar_type: ScalarType::Currency
        }
    );
}

#[test]
fn explicit_policies_and_other_kinds_are_not_silently_erased() {
    let zero = Value::Currency(Currency::from_scaled(0));
    for native in [
        CreateColumn::new("Amount", ScalarType::Currency).with_required(false),
        CreateColumn::new("Amount", ScalarType::Currency)
            .with_default_literal(&zero)
            .unwrap(),
        CreateColumn::new("Amount", ScalarType::Int32),
        CreateColumn::new("Amount", ScalarType::Guid),
    ] {
        assert!(matches!(
            captured_declared(native),
            Err(Error::SchemaSqlUnsupported)
        ));
    }
}
