use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::{Double, ExpressionValue, Single, Value};
use tablecraft_db::{MultivalueList, ProjectedRow};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, OutputType, QueryError, QueryLimits, QueryValue,
    Source, execute_collections_with_schema_discovery, prepare, source::MultivalueVisitor,
};
use tablecraft_schema::{Column, ScalarType, SchemaName};
use tablecraft_sql::{parse_query, query::Query};

struct Data {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    elements: Vec<Column>,
    rows: Vec<Vec<Value>>,
    lists: Vec<Vec<Value>>,
    others: Vec<Vec<Value>>,
    named: Query,
    dynamic: Query,
    collection_scans: Cell<usize>,
    scans: Cell<usize>,
    changed: Cell<bool>,
    fail_after_rows: bool,
}

impl Data {
    fn new() -> Self {
        Self {
            columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            unavailable: vec![SchemaName::new("Tags".into()).unwrap()],
            elements: vec![Column::new("Tags".into(), ScalarType::Int32, true).unwrap()],
            rows: vec![
                vec![Value::Int32(1)],
                vec![Value::Int32(2)],
                vec![Value::Int32(3)],
            ],
            lists: vec![
                vec![Value::Null, Value::Int32(100), Value::Int32(100)],
                vec![],
                vec![Value::Int32(200)],
            ],
            others: Vec::new(),
            collection_scans: Cell::new(0),
            dynamic: parse_query(
                "TRANSFORM Count(*) SELECT Id FROM T GROUP BY Id PIVOT IIf(Id=1,'A','B')",
                Default::default(),
            )
            .unwrap()
            .query()
            .clone(),
            named: parse_query("SELECT Id,Tags AS Items FROM T", Default::default())
                .unwrap()
                .query()
                .clone(),
            scans: Cell::new(0),
            changed: Cell::new(false),
            fail_after_rows: false,
        }
    }
}

impl Source for Data {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }

    fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(if self.changed.get() {
            &[]
        } else {
            &self.elements
        })
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok(if name.eq_ignore_ascii_case("Named") {
            Some(&self.named)
        } else if name.eq_ignore_ascii_case("DynamicPivot") {
            Some(&self.dynamic)
        } else {
            None
        })
    }

    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }

    fn scan_multivalues(
        &self,
        _: &str,
        scalars: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.collection_scans.set(self.collection_scans.get() + 1);
        for (position, (row, list)) in self.rows.iter().zip(&self.lists).enumerate() {
            let lists = collections
                .iter()
                .map(|index| match *index {
                    0 => list as &dyn MultivalueList,
                    1 => &self.others[position] as &dyn MultivalueList,
                    _ => panic!("unexpected collection request"),
                })
                .collect::<Vec<_>>();
            visitor(ProjectedRow::select(row, scalars)?, &lists)?;
        }
        if self.fail_after_rows {
            Err(QueryError::Source)
        } else {
            Ok(())
        }
    }
}

fn execute(sql: &str, data: &Data) -> Result<CollectionQueryResult, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    execute_collections_with_schema_discovery(
        parsed.query(),
        data,
        &[],
        ExecutionOptions::default(),
    )
}

fn scalar(value: i32) -> QueryValue {
    QueryValue::Scalar(ExpressionValue::Value(Value::Int32(value)))
}

fn list(value: &QueryValue) -> &[Value] {
    match value {
        QueryValue::Multivalue(value) => value.values(),
        QueryValue::Scalar(_) | QueryValue::AbsentMultivalue => {
            panic!("expected a structured list")
        }
    }
}

#[test]
fn complete_lists_keep_duplicates_null_children_and_empty_parent_lists() {
    let data = Data::new();
    let parsed = parse_query("SELECT * FROM T ORDER BY Id", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    assert_eq!(data.scans.get(), 0);
    assert!(matches!(
        plan.output_types(),
        Some([
            _,
            OutputType::Multivalue {
                element_nullable: true,
                ..
            }
        ])
    ));
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_eq!(result.rows().len(), 3);
    for (index, row) in result.rows().iter().enumerate() {
        assert_eq!(row[0], scalar(index as i32 + 1));
        assert_eq!(list(&row[1]), data.lists[index]);
    }
    let result = execute("SELECT Id,Tags FROM T WHERE Tags.Value=100", &data).unwrap();
    assert_eq!(result.rows().len(), 1);
    assert_eq!(list(&result.rows()[0][1]), data.lists[0]);
}

#[test]
fn aliases_named_and_derived_layers_keep_scalar_order_and_independent_top() {
    let data = Data::new();
    for sql in [
        "SELECT TOP 1 Id,Items FROM Named ORDER BY Id DESC",
        "SELECT TOP 1 Id,Items FROM (SELECT Id,Tags AS Items FROM T) AS Q ORDER BY Id DESC",
    ] {
        let result = execute(sql, &data).unwrap();
        assert_eq!(result.rows()[0][0], scalar(3));
        assert_eq!(list(&result.rows()[0][1]), &[Value::Int32(200)]);
        assert_eq!(result.rows().len(), 1);
    }
    let result = execute(
        "SELECT Tags,Id AS N,N AS Again FROM T ORDER BY 2 DESC",
        &data,
    )
    .unwrap();
    assert_eq!(result.rows()[0][1], scalar(3));
    assert_eq!(result.rows()[0][2], scalar(3));
    for order in ["N", "2"] {
        let sql = format!(
            "SELECT Tags AS FirstList,Id AS N,Tags AS SecondList,N+1 AS Next FROM T ORDER BY {order} DESC"
        );
        let result = execute(&sql, &data).unwrap();
        assert_eq!(result.rows()[0][1], scalar(3));
        assert_eq!(list(&result.rows()[0][0]), &[Value::Int32(200)]);
        assert_eq!(list(&result.rows()[0][2]), &[Value::Int32(200)]);
        assert_eq!(
            result.rows()[0][3],
            QueryValue::Scalar(ExpressionValue::Value(Value::Int64(4)))
        );
    }
}

#[test]
fn scalar_apis_and_unsupported_collection_operations_refuse_before_scans() {
    let data = Data::new();
    let parsed = parse_query("SELECT Tags FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    assert_eq!(
        plan.execute(&data, &[], Default::default()).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(
        plan.execute_expression(&data, &[], Default::default())
            .err(),
        Some(QueryError::Unsupported)
    );
    for sql in [
        "SELECT Count(Tags) FROM T",
        "SELECT Tags+1 FROM T",
        "SELECT Tags FROM T ORDER BY Tags",
        "SELECT Tags FROM T GROUP BY Tags",
        "SELECT DISTINCT Tags FROM T",
        "SELECT Tags FROM T UNION ALL SELECT Tags FROM T",
    ] {
        assert_eq!(
            execute(sql, &data).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    assert_eq!(data.scans.get(), 0);
}

#[test]
fn schema_child_suffix_and_source_eof_failures_never_publish_partial_lists() {
    let mut data = Data::new();
    let parsed = parse_query("SELECT Tags FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    data.changed.set(true);
    assert_eq!(
        plan.execute_collections(&data, &[], Default::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    assert_eq!(data.scans.get(), 0);
    data.changed.set(false);
    data.lists[0].push(Value::Text("invalid stored child".into()));
    assert_eq!(
        execute("SELECT Tags FROM T WHERE Tags.Value=100", &data).err(),
        Some(QueryError::Source)
    );
    data.lists[0].pop();
    data.fail_after_rows = true;
    assert_eq!(
        execute("SELECT Tags FROM T", &data).err(),
        Some(QueryError::Source)
    );
    data.fail_after_rows = false;
    assert_eq!(
        execute("SELECT Tags FROM T", &data).unwrap().rows().len(),
        3
    );
}

#[test]
fn scalar_only_collection_api_keeps_eager_errors_and_shared_resource_limits() {
    let data = Data::new();
    let result = execute("SELECT Id FROM T ORDER BY Id", &data).unwrap();
    assert_eq!(
        result.rows(),
        &[vec![scalar(1)], vec![scalar(2)], vec![scalar(3)]]
    );
    let parsed = parse_query("SELECT Tags FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    let cancelled = || true;
    assert_eq!(
        plan.execute_collections(
            &data,
            &[],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            }
        )
        .err(),
        Some(QueryError::Cancelled)
    );
    let result = plan.execute_collections(
        &data,
        &[],
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    );
    assert!(result.is_err());
    assert_eq!(
        execute("SELECT Id,1/0 FROM T", &data).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

#[test]
fn collection_metadata_positions_are_independent_of_unavailable_name_order() {
    for same_type in [false, true] {
        let mut data = Data::new();
        data.unavailable
            .insert(0, SchemaName::new("Other".into()).unwrap());
        let kind = if same_type {
            ScalarType::Int32
        } else {
            ScalarType::Text
        };
        data.elements
            .push(Column::new("Other".into(), kind, true).unwrap());
        data.others = (0..3)
            .map(|index| {
                if same_type {
                    vec![Value::Int32(700 + index)]
                } else {
                    vec![Value::Text(format!("other-{index}"))]
                }
            })
            .collect();
        for (sql, positions) in [
            ("SELECT Tags,Other FROM T", [0, 1]),
            ("SELECT * FROM T", [2, 1]),
            (
                "SELECT B,A FROM (SELECT Tags AS A,Other AS B FROM T) AS Q",
                [1, 0],
            ),
        ] {
            let result = execute(sql, &data).unwrap();
            for (index, row) in result.rows().iter().enumerate() {
                assert_eq!(list(&row[positions[0]]), data.lists[index]);
                assert_eq!(list(&row[positions[1]]), data.others[index]);
            }
        }
        let parsed = parse_query("SELECT Tags,Other FROM T", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &data).unwrap();
        let scans = data.collection_scans.get();
        data.elements.swap(0, 1);
        assert_eq!(
            plan.execute_collections(&data, &[], Default::default())
                .err(),
            Some(QueryError::SchemaChanged)
        );
        assert_eq!(data.collection_scans.get(), scans);
    }
}

#[test]
fn duplicate_collection_and_scalar_aliases_keep_the_existing_ambiguity_boundary() {
    let data = Data::new();
    assert_eq!(
        execute("SELECT Tags AS X,Id AS X,X+1 FROM T", &data).err(),
        Some(QueryError::AmbiguousName)
    );
    assert_eq!(
        execute("SELECT Tags AS X,X FROM T", &data).err(),
        Some(QueryError::Unsupported)
    );
    assert_eq!(data.scans.get(), 0);
}

#[test]
fn authorized_scalar_discovery_can_precede_typed_admission_but_never_collection_scans() {
    struct Ordinal;
    impl Collation for Ordinal {
        fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
            Ok(a.cmp(b))
        }
    }
    let data = Data::new();
    let parsed = parse_query(
        "SELECT DISTINCT T.Tags FROM T LEFT JOIN DynamicPivot AS P ON T.Id=P.Id",
        Default::default(),
    )
    .unwrap();
    let result = execute_collections_with_schema_discovery(
        parsed.query(),
        &data,
        &[],
        ExecutionOptions {
            collation: Some(&Ordinal),
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::Unsupported));
    assert!(data.scans.get() > 0);
    assert_eq!(data.collection_scans.get(), 0);
}

#[test]
fn complete_stored_float_lists_preserve_ieee_bits_without_scalar_admission() {
    for (kind, values) in [
        (
            ScalarType::Single,
            [0, 0x80000000, 1, 0x7f800000, 0x7fc00001, 0xffc12345]
                .map(|bits| Value::Single(Single::from_bits(bits)))
                .to_vec(),
        ),
        (
            ScalarType::Double,
            [
                0,
                0x8000000000000000,
                1,
                0x7ff0000000000000,
                0x7ff8000000000001,
                0xfff8123456789abc,
            ]
            .map(|bits| Value::Double(Double::from_bits(bits)))
            .to_vec(),
        ),
    ] {
        let mut data = Data::new();
        data.elements[0] = Column::new("Tags".into(), kind, true).unwrap();
        data.lists = vec![values.clone(), vec![], vec![Value::Null]];
        let result = execute(
            "SELECT Items FROM (SELECT Tags AS Items FROM T) AS Q",
            &data,
        )
        .unwrap();
        assert_eq!(list(&result.rows()[0][0]), values);
        let parsed = parse_query("SELECT Tags.Value FROM T", Default::default()).unwrap();
        let plan = prepare(parsed.query(), &data).unwrap();
        assert_eq!(
            plan.execute(&data, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::NonFinite))
        );
    }
}

#[test]
fn authorized_scalar_crosstab_dependency_can_join_complete_lists_after_discovery() {
    struct Ordinal;
    impl Collation for Ordinal {
        fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
            Ok(a.cmp(b))
        }
    }
    let data = Data::new();
    let parsed = parse_query(
        "SELECT T.Id,T.Tags FROM T INNER JOIN DynamicPivot AS P ON T.Id=P.Id ORDER BY T.Id",
        Default::default(),
    )
    .unwrap();
    let result = execute_collections_with_schema_discovery(
        parsed.query(),
        &data,
        &[],
        ExecutionOptions {
            collation: Some(&Ordinal),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.rows().len(), 3);
    for (index, row) in result.rows().iter().enumerate() {
        assert_eq!(row[0], scalar(index as i32 + 1));
        assert_eq!(list(&row[1]), data.lists[index]);
    }
    assert!(data.scans.get() > 0);
    assert_eq!(data.collection_scans.get(), 1);
}
