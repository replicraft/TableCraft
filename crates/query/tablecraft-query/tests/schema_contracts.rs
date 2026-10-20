use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{
    Column, NameComparison, ScalarType, SchemaError, TypeDescriptor, validate_columns,
};
use tablecraft_sql::{ParseLimits, parse_query};

#[test]
fn query_limits_remain_explicit_when_generic_schemas_are_larger() {
    let long = Column::new("x".repeat(300), ScalarType::Int64, false).unwrap();
    assert!(validate_columns(std::slice::from_ref(&long), NameComparison::Exact).is_ok());
    assert!(matches!(
        MemoryTable::new("T".into(), vec![long], vec![]),
        Err(QueryError::Schema(SchemaError::Name))
    ));
    assert!(matches!(
        MemoryTable::new(
            "x".repeat(300),
            vec![Column::new("ID".into(), ScalarType::Int64, false).unwrap()],
            vec![]
        ),
        Err(QueryError::Schema(SchemaError::Name))
    ));
    let columns = (0..257)
        .map(|i| Column::new(format!("C{i}"), ScalarType::Int64, false).unwrap())
        .collect::<Vec<_>>();
    assert!(validate_columns(&columns, NameComparison::Exact).is_ok());
    struct Wide {
        columns: Vec<Column>,
    }
    impl Source for Wide {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("invalid profile must fail before scanning")
        }
    }
    let source = Wide { columns };
    let parsed = parse_query("SELECT * FROM T", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(parsed.query(), &source).err(),
        Some(QueryError::Schema(SchemaError::Width))
    );
}

fn text_source(max: usize, value: &str) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::with_type("Label".into(), TypeDescriptor::text(max).unwrap(), false)
                    .unwrap(),
            ],
            vec![vec![Value::Text(value.into())]],
        )
        .unwrap(),
    ])
    .unwrap()
}
#[test]
fn fields_and_parameters_share_declared_text_constraints() {
    assert!(
        MemoryTable::new(
            "T".into(),
            vec![
                Column::with_type("Label".into(), TypeDescriptor::text(2).unwrap(), false).unwrap()
            ],
            vec![vec![Value::Text("abc".into())]]
        )
        .is_err()
    );
    let source = text_source(2, "é🦀");
    let parsed = parse_query(
        "PARAMETERS P TEXT(2); SELECT Label, P FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let result = plan
        .execute(
            &source,
            &[("p", Value::Text("é🦀".into()))],
            ExecutionOptions::default(),
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Text("é🦀".into()), Value::Text("é🦀".into())]]
    );
    assert_eq!(
        plan.execute(
            &source,
            &[("p", Value::Text("abc".into()))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
    assert_eq!(
        plan.execute(
            &source,
            &[("p", Value::Int64(2))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::Parameter)
    );
    let changed = text_source(3, "abc");
    assert_eq!(
        plan.execute(&changed, &[("p", Value::Null)], ExecutionOptions::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
}

#[test]
fn provider_rows_cannot_bypass_declared_field_constraints() {
    struct Invalid {
        columns: Vec<Column>,
    }
    impl Source for Invalid {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            visit: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            visit(&[Value::Text("abc".into())])
        }
    }
    let source = Invalid {
        columns: vec![
            Column::with_type("Label".into(), TypeDescriptor::text(2).unwrap(), false).unwrap(),
        ],
    };
    let query = parse_query("SELECT Label FROM T", ParseLimits::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Schema(SchemaError::Row))
    );
}

#[test]
fn decimal_field_constraints_survive_binding_and_reject_provider_drift() {
    use tablecraft_core::values::Decimal;
    struct DecimalSource {
        columns: Vec<Column>,
        value: Value,
    }
    impl Source for DecimalSource {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            visit: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            visit(std::slice::from_ref(&self.value))
        }
    }
    let source = |precision, scale, value| DecimalSource {
        columns: vec![
            Column::with_type(
                "Amount".into(),
                TypeDescriptor::decimal(precision, scale).unwrap(),
                true,
            )
            .unwrap(),
        ],
        value,
    };
    let decimal = |coefficient, scale| Value::Decimal(Decimal::new(coefficient, scale).unwrap());
    let initial = source(4, 2, decimal(9999, 2));
    let query = parse_query("SELECT Amount FROM T", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &initial).unwrap();
    assert_eq!(
        plan.execute(&initial, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        &[vec![decimal(9999, 2)]]
    );
    for value in [
        decimal(10000, 2),
        decimal(-10000, 2),
        decimal(1, 1),
        decimal(0, 0),
    ] {
        assert_eq!(
            plan.execute(&source(4, 2, value), &[], ExecutionOptions::default())
                .err(),
            Some(QueryError::Schema(SchemaError::Row))
        );
    }
    assert_eq!(
        plan.execute(
            &source(5, 2, decimal(9999, 2)),
            &[],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::SchemaChanged)
    );
    assert_eq!(
        plan.execute(
            &source(4, 3, decimal(9999, 3)),
            &[],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::SchemaChanged)
    );
    assert_eq!(
        plan.execute(&source(4, 2, Value::Null), &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        &[vec![Value::Null]]
    );
}
