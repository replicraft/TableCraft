//! Compound aliases reuse existing parameter carriers and conservative constraints.
use tablecraft_core::values::Value;
use tablecraft_query::{
    ParameterType, QueryError, prepare, resolve_parameter_type, source::MemorySource,
};
use tablecraft_schema::ScalarType;
use tablecraft_sql::parse_query;

#[test]
fn compound_character_parameters_keep_declared_limits_and_exact_values() {
    let source = MemorySource::default();
    for alias in [
        "CHARACTER VARYING",
        "NATIONAL CHARACTER",
        "NATIONAL CHAR",
        "NATIONAL CHARACTER VARYING",
        "NATIONAL CHAR VARYING",
    ] {
        let sql = format!("PARAMETERS P {alias}(7); SELECT P AS Result");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let ParameterType::Scalar(declaration) =
            resolve_parameter_type(&parsed.query().parameters[0].data_type).unwrap()
        else {
            panic!("Text declaration");
        };
        assert_eq!(declaration.scalar_type(), ScalarType::Text);
        assert_eq!(declaration.max_text_characters(), Some(7));
        let prepared = prepare(parsed.query(), &source).unwrap();
        for value in [
            Value::Text("雪🙂".into()),
            Value::Text(String::new()),
            Value::Null,
        ] {
            let parameters = [("P", value)];
            let result = prepared
                .execute(&source, &parameters, Default::default())
                .unwrap();
            assert_eq!(&result.rows()[0][0], &parameters[0].1, "{alias}");
        }
        assert!(
            prepared
                .execute(
                    &source,
                    &[("P", Value::Text("12345678".into()))],
                    Default::default()
                )
                .is_err()
        );
    }
}

#[test]
fn varying_binary_parameters_do_not_confuse_native_boolean_bit_or_invent_a_length_policy() {
    let source = MemorySource::default();
    for alias in ["BINARY VARYING", "BIT VARYING"] {
        let parsed = parse_query(
            &format!("PARAMETERS P {alias}; SELECT P"),
            Default::default(),
        )
        .unwrap();
        let ParameterType::Scalar(declaration) =
            resolve_parameter_type(&parsed.query().parameters[0].data_type).unwrap()
        else {
            panic!("Binary declaration");
        };
        assert_eq!(declaration.scalar_type(), ScalarType::Binary);
        let prepared = prepare(parsed.query(), &source).unwrap();
        let value = Value::Binary(vec![0, 255, 0, 127]);
        let parameters = [("P", value)];
        assert_eq!(
            &prepared
                .execute(&source, &parameters, Default::default())
                .unwrap()
                .rows()[0][0],
            &parameters[0].1
        );
        let sized = parse_query(
            &format!("PARAMETERS P {alias}(16); SELECT P"),
            Default::default(),
        )
        .unwrap();
        assert!(
            matches!(
                resolve_parameter_type(&sized.query().parameters[0].data_type),
                Err(QueryError::Unsupported)
            ),
            "existing Binary parameters have no length-constraint owner"
        );
    }
    let parsed = parse_query("PARAMETERS P BIT; SELECT P", Default::default()).unwrap();
    let ParameterType::Scalar(declaration) =
        resolve_parameter_type(&parsed.query().parameters[0].data_type).unwrap()
    else {
        panic!("Boolean declaration");
    };
    assert_eq!(declaration.scalar_type(), ScalarType::Boolean);
}
