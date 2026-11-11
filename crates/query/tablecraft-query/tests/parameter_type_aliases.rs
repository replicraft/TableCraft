use tablecraft_core::values::{Double, Single, Value};
use tablecraft_query::{ParameterType, prepare, resolve_parameter_type, source::MemorySource};
use tablecraft_schema::ScalarType;
use tablecraft_sql::parse_query;

#[test]
fn documented_aliases_bind_and_execute_exact_parameter_carriers() {
    let source = MemorySource::default();
    for (alias, kind, value) in [
        ("LOGICAL", ScalarType::Boolean, Value::Boolean(true)),
        ("LOGICAL1", ScalarType::Boolean, Value::Boolean(false)),
        ("INTEGER1", ScalarType::Byte, Value::Byte(255)),
        ("INTEGER2", ScalarType::Int16, Value::Int16(-32768)),
        ("INTEGER4", ScalarType::Int32, Value::Int32(2147483647)),
        (
            "FLOAT4",
            ScalarType::Single,
            Value::Single(Single::from_float(1.25)),
        ),
        (
            "FLOAT8",
            ScalarType::Double,
            Value::Double(Double::from_float(-2.5)),
        ),
        (
            "NUMBER",
            ScalarType::Double,
            Value::Double(Double::from_float(3.75)),
        ),
        ("ALPHANUMERIC", ScalarType::Text, Value::Text("α".into())),
        ("STRING", ScalarType::Text, Value::Text(String::new())),
        ("NCHAR", ScalarType::Text, Value::Text("漢字".into())),
        ("NOTE", ScalarType::Text, Value::Text("one\ntwo".into())),
        ("NTEXT", ScalarType::Text, Value::Text("🦀".into())),
        ("VARBINARY", ScalarType::Binary, Value::Binary(vec![0, 255])),
        ("GENERAL", ScalarType::Binary, Value::Binary(vec![1, 0])),
        (
            "OLEOBJECT",
            ScalarType::Binary,
            Value::Binary(vec![255, 0, 127]),
        ),
    ] {
        let sql = format!("PARAMETERS P {alias}; SELECT P AS Result");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let ParameterType::Scalar(declaration) =
            resolve_parameter_type(&parsed.query().parameters[0].data_type).unwrap()
        else {
            panic!("expected the documented scalar alias {alias}");
        };
        assert_eq!(declaration.scalar_type(), kind, "{alias}");
        let plan = prepare(parsed.query(), &source).unwrap();
        let parameters = [("P", value)];
        let result = plan
            .execute(&source, &parameters, Default::default())
            .unwrap();
        assert_eq!(result.rows().len(), 1, "{alias}");
        assert_eq!(&result.rows()[0][0], &parameters[0].1, "{alias}");
    }
}

#[test]
fn decimal_alias_keeps_declared_precision_and_scale() {
    let parsed = parse_query("PARAMETERS P DEC(12,4); SELECT P", Default::default()).unwrap();
    let ParameterType::Scalar(declaration) =
        resolve_parameter_type(&parsed.query().parameters[0].data_type).unwrap()
    else {
        panic!("expected a constrained Decimal parameter");
    };
    assert_eq!(declaration.scalar_type(), ScalarType::Decimal);
    assert_eq!(declaration.decimal_constraints().unwrap().precision(), 12);
    assert_eq!(declaration.decimal_constraints().unwrap().scale(), 4);
}
