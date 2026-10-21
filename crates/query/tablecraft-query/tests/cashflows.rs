use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            ["Unused", "N"]
                .map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap())
                .to_vec(),
            [1, 2, 4]
                .into_iter()
                .map(|n| vec![Value::Int32(999), Value::Int32(n)])
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
        .execute(&data, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn binding_arrays_reads_metadata_without_visiting_records() {
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
            panic!("binding must not scan records");
        }
    }

    let parsed = parse_query(
        "SELECT NPV(0,Array(N,1)),IRR(Array(-100,N)) FROM T",
        Default::default(),
    )
    .unwrap();

    prepare(parsed.query(), &MetadataOnly(source())).unwrap();
}

fn near(rows: &[Vec<Value>], expected: &[f64]) {
    assert_eq!(rows.len(), expected.len());
    for (row, expected) in rows.iter().zip(expected) {
        let [Value::Double(value)] = row.as_slice() else {
            panic!("expected single Double");
        };
        assert!(
            (value.to_float() - expected).abs() <= 1e-10 * expected.abs().max(1.),
            "{value:?} != {expected}"
        );
    }
}

#[test]
fn cashflows_retain_dependencies_group_identity_domains_and_correlated_scopes() {
    near(
        &run("SELECT NPV(0,Array(N,0,N*10)) FROM T ORDER BY N").unwrap(),
        &[11., 22., 44.],
    );
    near(
        &run("SELECT NPV(0,Array(N,1)) FROM T GROUP BY NPV(0,Array(N,1)) ORDER BY 1").unwrap(),
        &[2., 3., 5.],
    );
    near(
        &run("SELECT IRR(Array(-100,Sum(N)*20)) FROM T").unwrap(),
        &[0.4],
    );
    near(
        &run("SELECT DSum('NPV(0,Array(N,1))','T')").unwrap(),
        &[10.],
    );
    near(
        &run("SELECT (SELECT NPV(0,Array(T.N,1))) FROM T ORDER BY N").unwrap(),
        &[2., 3., 5.],
    );
    near(
        &run("SELECT MIRR(Array(-100,121),0,0) UNION ALL SELECT IRR(VBA.Array(-100,121),)")
            .unwrap(),
        &[0.21, 0.21],
    );
    near(
        &run("SELECT NPV(0,Array(N,1)) AS V FROM T WHERE NPV(0,Array(N,1))>2 ORDER BY V").unwrap(),
        &[3., 5.],
    );

    // Identical flattened operands with a different array range are distinct keys.
    assert!(run("SELECT IRR(Array(-100,N),0) FROM T GROUP BY IRR(Array(-100,N,0))").is_err());
}

#[test]
fn parameter_plans_reuse_explicit_arrays_and_reject_late_failures() {
    let data = source();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT NPV(0,Array(P,1))",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &data).unwrap();
    for parameter in [1, 20] {
        let result = plan
            .execute(&data, &[("P", Value::Int32(parameter))], Default::default())
            .unwrap();
        near(result.rows(), &[f64::from(parameter + 1)]);
    }

    for sql in [
        "SELECT NPV(Null,Array(1,1/0))",
        "SELECT IRR(Array(Null,1/0))",
        "SELECT MIRR(Array(Null,1),0,1/0)",
        "SELECT TOP 1 NPV(0,Array(N,1/(N-2))) FROM T",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT Array(1,2)",
        "SELECT NPV(0,N) FROM T",
        "SELECT IRR(Array(-1,,2))",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported), "{sql}");
    }
}

#[test]
fn constructor_and_iteration_work_share_limits_and_all_cancellation_checkpoints() {
    let data = source();
    let parsed = parse_query("SELECT IRR(Array(-100,0,121))", Default::default()).unwrap();
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
                    steps: 12,
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
