use super::*;

#[test]
fn collation_is_explicit_and_representational_equality_is_not_sql_equality() {
    let empty = MemorySource::default();
    let query = parse_query(
        "SELECT 'a' AS S UNION SELECT 'A' ORDER BY S",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &empty).unwrap();
    assert_eq!(
        plan.execute(&empty, &[], ExecutionOptions::default()).err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
    let result = plan
        .execute(
            &empty,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Text("A".into())], vec![Value::Text("a".into())]]
    );
    let exact = MemorySource::new(vec![table(
        "D",
        &[("V", ScalarType::Decimal)],
        vec![
            vec![Value::Decimal(Decimal::new(10, 1).unwrap())],
            vec![Value::Decimal(Decimal::new(100, 2).unwrap())],
        ],
    )])
    .unwrap();
    assert_eq!(
        run("SELECT DISTINCT V FROM D", &exact)
            .unwrap()
            .rows()
            .len(),
        1
    );
    let money = MemorySource::new(vec![table(
        "M",
        &[("V", ScalarType::Currency)],
        vec![
            vec![Value::Currency(Currency::from_scaled(123))],
            vec![Value::Currency(Currency::from_scaled(234))],
        ],
    )])
    .unwrap();
    assert_eq!(
        run("SELECT Sum(V) FROM M", &money).unwrap().rows(),
        &[vec![Value::Currency(Currency::from_scaled(357))]]
    );
}
#[test]
fn limits_cancellation_source_failures_and_schema_changes_are_observable() {
    let source = source();
    let query = parse_query("SELECT ID FROM Items ORDER BY ID", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            rows: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| true),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
    let changed = MemorySource::new(vec![table(
        "Items",
        &[("Different", ScalarType::Int64)],
        vec![],
    )])
    .unwrap();
    assert_eq!(
        plan.execute(&changed, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    struct Broken {
        columns: Vec<Column>,
    }
    impl Source for Broken {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            visitor(&[Value::Text("secret".into())])
        }
    }
    let broken = Broken {
        columns: vec![Column::new("ID".into(), ScalarType::Int64, true).unwrap()],
    };
    assert!(matches!(
        run("SELECT ID FROM T", &broken),
        Err(QueryError::Schema(_))
    ));
}

#[test]
fn execution_retains_visitor_rejection_and_checks_cancellation_mid_scan() {
    struct IgnoresError {
        columns: Vec<Column>,
    }
    impl Source for IgnoresError {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            let _ = visitor(&[Value::Text("hidden".into())]);
            let _ = visitor(&[Value::Int64(1)]);
            Ok(())
        }
    }
    let bad = IgnoresError {
        columns: vec![Column::new("ID".into(), ScalarType::Int64, true).unwrap()],
    };
    assert!(matches!(
        run("SELECT ID FROM T", &bad),
        Err(QueryError::Schema(_))
    ));
    let source = source();
    let query = parse_query(
        "SELECT a.ID FROM Items AS a, Items AS b ORDER BY a.ID",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let checks = Cell::new(0);
    let cancel = || {
        checks.set(checks.get() + 1);
        checks.get() > 20
    };
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancel),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
    assert!(checks.get() > 20);
}

#[test]
fn source_error_after_rows_is_not_a_successful_partial_result() {
    struct Fails {
        columns: Vec<Column>,
    }
    impl Source for Fails {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.columns)
        }
        fn scan(
            &self,
            _: &str,
            visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            visitor(&[Value::Int64(1)])?;
            Err(QueryError::Source)
        }
    }
    let source = Fails {
        columns: vec![Column::new("ID".into(), ScalarType::Int64, true).unwrap()],
    };
    assert_eq!(
        run("SELECT ID FROM T", &source).err(),
        Some(QueryError::Source)
    );
}
