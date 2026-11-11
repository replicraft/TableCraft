use super::declaration;
use crate::requests::validate;
use serde_json::json;
use tablecraft_builders::table::FieldAdditionDeclaration;
use tablecraft_schema::ScalarType;

#[test]
fn twelve_type_declarations_reuse_validated_builders_and_preserve_existing_requests() {
    assert_eq!(
        declaration(&json!({"document_revision":1,"table":"T","column":"C","kind":"number"}))
            .unwrap(),
        FieldAdditionDeclaration::NumberLongZero
    );
    for (choice, scalar) in [
        (json!({"kind":"yes_no"}), ScalarType::Boolean),
        (json!({"kind":"currency"}), ScalarType::Currency),
        (json!({"kind":"large_integer"}), ScalarType::Int64),
        (json!({"kind":"date_time"}), ScalarType::DateTime),
        (
            json!({"kind":"number","number_size":"byte"}),
            ScalarType::Byte,
        ),
        (
            json!({"kind":"number","number_size":"integer"}),
            ScalarType::Int16,
        ),
        (
            json!({"kind":"number","number_size":"long_integer"}),
            ScalarType::Int32,
        ),
        (
            json!({"kind":"number","number_size":"single"}),
            ScalarType::Single,
        ),
        (
            json!({"kind":"number","number_size":"double"}),
            ScalarType::Double,
        ),
        (
            json!({"kind":"number","number_size":"decimal","precision":28,"scale":4}),
            ScalarType::Decimal,
        ),
        (
            json!({"kind":"number","number_size":"replication_id"}),
            ScalarType::Guid,
        ),
        (
            json!({"kind":"short_text","max_utf16_units":80,"unicode_compression":false}),
            ScalarType::Text,
        ),
    ] {
        let mut request = choice;
        request["document_revision"] = json!(1);
        request["table"] = json!("T");
        request["column"] = json!("C");
        assert!(
            validate("review_field_addition", &request).is_ok(),
            "{request}"
        );
        match declaration(&request).unwrap() {
            FieldAdditionDeclaration::Typed(value) => {
                assert_eq!(value.descriptor().scalar_type(), scalar);
                if scalar == ScalarType::Decimal {
                    let constraints = value.descriptor().decimal_constraints().unwrap();
                    assert_eq!((constraints.precision(), constraints.scale()), (28, 4));
                }
            }
            FieldAdditionDeclaration::ShortText {
                max_utf16_units,
                unicode_compression,
            } => {
                assert_eq!(scalar, ScalarType::Text);
                assert_eq!(max_utf16_units.get(), 80);
                assert!(!unicode_compression);
            }
            FieldAdditionDeclaration::NumberLongZero => {
                panic!("explicit type selection must retain its typed declaration")
            }
        }
    }
}

#[test]
fn incompatible_or_invalid_type_parameters_refuse_before_database_dispatch() {
    for choice in [
        json!({"kind":"currency","number_size":"byte"}),
        json!({"kind":"number","max_utf16_units":80}),
        json!({"kind":"number","precision":28}),
        json!({"kind":"number","number_size":"byte","scale":1}),
        json!({"kind":"number","number_size":"decimal","precision":29}),
        json!({"kind":"number","number_size":"decimal","precision":2,"scale":3}),
        json!({"kind":"short_text","max_utf16_units":0}),
        json!({"kind":"short_text","max_utf16_units":256}),
        json!({"kind":"yes_no","required":true}),
        json!({"kind":"number","default_expression":"Now()"}),
    ] {
        let mut request = choice;
        request["document_revision"] = json!(1);
        request["table"] = json!("T");
        request["column"] = json!("C");
        assert!(
            validate("review_field_addition", &request).is_err(),
            "{request}"
        );
    }
}
