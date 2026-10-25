use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct BinaryCollation;

impl Collation for BinaryCollation {
    fn compare(&self, left: &str, right: &str) -> Result<std::cmp::Ordering, EvalError> {
        Ok(left.encode_utf16().cmp(right.encode_utf16()))
    }
}

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            [
                Column::new("Unused".into(), ScalarType::Int32, false).unwrap(),
                Column::new("N".into(), ScalarType::Int32, false).unwrap(),
                Column::new("B".into(), ScalarType::Binary, false).unwrap(),
            ]
            .to_vec(),
            [1, 2, 4]
                .into_iter()
                .map(|n| {
                    vec![
                        Value::Int32(999),
                        Value::Int32(n),
                        Value::Binary(vec![0xff, 0]),
                    ]
                })
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let data = source();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &data)?
        .execute(
            &data,
            &[],
            ExecutionOptions {
                collation: Some(&BinaryCollation),
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn metadata_binding_never_visits_rows_or_infers_array_element_types() {
    struct MetadataOnly(MemorySource);

    impl Source for MetadataOnly {
        fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
            self.0.columns(table)
        }

        fn scan(
            &self,
            _table: &str,
            _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("binding must not scan rows");
        }
    }

    let parsed = parse_query(
        "SELECT TypeName(Array(N)),VarType(B),UBound(Array(N,Null)) FROM T",
        Default::default(),
    )
    .unwrap();
    prepare(parsed.query(), &MetadataOnly(source())).unwrap();
}

#[test]
fn type_inspection_retains_dependencies_scopes_group_identity_and_binary_arrays() {
    assert_eq!(
        run("SELECT TypeName(Array(Sum(N))),UBound(Array(Sum(N),Count(*))) FROM T").unwrap(),
        [vec![Value::Text("Variant()".into()), Value::Int32(1)]]
    );
    assert_eq!(
        run("SELECT TypeName(N),VarType(N),IsArray(B),TypeName(B),VarType(B),UBound(B) FROM T")
            .unwrap(),
        vec![
            vec![
                Value::Text("Long".into()),
                Value::Int16(3),
                Value::Boolean(true),
                Value::Text("Byte()".into()),
                Value::Int16(8209),
                Value::Int32(1)
            ];
            3
        ]
    );
    assert_eq!(
        run("SELECT N FROM T WHERE IsArray(Array(N)) ORDER BY N").unwrap(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(4)]
        ]
    );
    assert_eq!(
        run("SELECT (SELECT UBound(Array(T.N,Null),T.N)) FROM T WHERE N=1").unwrap(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(
        run("SELECT VarType(Sum(N)),DCount('TypeName(Array(N))','T') FROM T").unwrap(),
        [vec![Value::Int16(20), Value::Int64(3)]]
    );
    assert_eq!(
        run("SELECT TypeName(Array(N)) FROM T GROUP BY TypeName(Array(N))").unwrap(),
        [vec![Value::Text("Variant()".into())]]
    );
    assert_eq!(
        run("SELECT UBound(Array(N),1) FROM T GROUP BY UBound(Array(N),1)").unwrap(),
        [vec![Value::Int32(0)]]
    );
    assert!(run("SELECT LBound(Array(N)) FROM T GROUP BY UBound(Array(N))").is_err());
    assert!(run("SELECT TypeName(Array(N)) FROM T GROUP BY VarType(Array(N))").is_err());
}

#[test]
fn eager_arrays_and_dimensions_preserve_errors_and_reusable_parameter_types() {
    let data = source();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT VarType(P),LBound(Array(P),)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    for p in [0, 12] {
        assert_eq!(
            plan.execute(&data, &[("P", Value::Int32(p))], Default::default())
                .unwrap()
                .rows(),
            [vec![Value::Int16(3), Value::Int32(0)]]
        );
    }

    for sql in [
        "SELECT IsError(1/0)",
        "SELECT TOP 1 IsArray(Array(N,1/(N-2))) FROM T",
        "SELECT UBound(Null,1/0)",
        "SELECT TypeName(Array(Null,1/0))",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT Array(N) FROM T",
        "SELECT TypeName(Array(Array(N))) FROM T",
        "SELECT UBound(Other.Array(1))",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported), "{sql}");
    }
}

#[test]
fn array_materialization_and_name_publication_share_query_limits_and_cancellation() {
    let data = source();
    let parsed = parse_query("SELECT TypeName(Array(1,2))", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    let calls = Cell::new(0);
    plan.execute(
        &data,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                calls.set(calls.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=calls.get() {
        let count = Cell::new(0);
        let error = plan
            .execute(
                &data,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| {
                        count.set(count.get() + 1);
                        count.get() == stop
                    }),
                    ..Default::default()
                },
            )
            .err();
        assert!(
            matches!(
                error,
                Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "{stop}: {error:?}"
        );
    }

    let error = plan
        .execute(
            &data,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .err();
    assert!(matches!(
        error,
        Some(QueryError::Limit(_) | QueryError::Evaluation(EvalError::Limit(_)))
    ));
}

#[path = "value_inspection/bounds_metadata.rs"]
mod bounds_metadata;
