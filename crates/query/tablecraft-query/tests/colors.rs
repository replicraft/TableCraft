use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            [1, 2, 4].map(|n| vec![Value::Int32(n)]).to_vec(),
        )
        .unwrap(),
    ])
    .unwrap()
}

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = source();
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), &source)?
        .execute(&source, &[], Default::default())?
        .rows()
        .to_vec())
}

#[test]
fn color_calls_preserve_projection_dependencies_scopes_and_group_identity() {
    assert_eq!(
        run("SELECT RGB(N,N+1,N+2),QBColor(N) FROM T ORDER BY N").unwrap(),
        [
            vec![Value::Int32(197121), Value::Int32(8388608)],
            vec![Value::Int32(262914), Value::Int32(32768)],
            vec![Value::Int32(394500), Value::Int32(128)],
        ]
    );
    assert_eq!(
        run("SELECT RGB(N,0,0),Count(*) FROM T GROUP BY RGB(N,0,0) ORDER BY 1").unwrap(),
        [1, 2, 4].map(|n| vec![Value::Int32(n), Value::Int64(1)])
    );
    assert_eq!(
        run("SELECT (SELECT RGB(T.N,0,0)) FROM T WHERE QBColor(N)<1000 ORDER BY N").unwrap(),
        [vec![Value::Int32(4)]]
    );
    assert_eq!(
        run("SELECT RGB(Sum(N),0,0),DSum('RGB(N,0,0)','T') FROM T").unwrap(),
        [vec![Value::Int32(7), Value::Int64(7)]]
    );
    assert_eq!(
        run("SELECT RGB(N,0,0) FROM T GROUP BY RGB(0,N,0)").err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn parameter_plans_and_eager_failures_retain_existing_boundaries() {
    let source = source();
    let parsed = parse_query(
        "PARAMETERS P LONG; SELECT RGB(P,0,0),QBColor(P)",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (parameter, packed) in [(1, 8388608), (2, 32768)] {
        let result = plan
            .execute(
                &source,
                &[("P", Value::Int32(parameter))],
                Default::default(),
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int32(parameter), Value::Int32(packed)]]
        );
    }

    for sql in [
        "SELECT RGB(Null,1/0,0)",
        "SELECT IIf(False,QBColor(1/0),0)",
        "SELECT TOP 1 RGB(N,1/((N-2)*(N-2)),0) FROM T",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(EvalError::DivisionByZero.into()),
            "{sql}"
        );
    }
    for sql in [
        "SELECT RGB(1,2)",
        "SELECT RGB(1,2,3,4)",
        "SELECT QBColor(*)",
        "SELECT QBColor(,)",
        "SELECT VBA.RGB(1,2,3)",
        "SELECT Round(1,2,3)",
    ] {
        assert_eq!(run(sql).err(), Some(QueryError::Unsupported), "{sql}");
    }
}

#[test]
fn binding_reads_metadata_only_and_every_execution_checkpoint_can_cancel() {
    struct MetadataOnly(MemorySource);

    impl Source for MetadataOnly {
        fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
            self.0.columns(name)
        }

        fn scan(
            &self,
            _name: &str,
            _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("binding must not inspect records");
        }
    }

    let source = source();
    let parsed = parse_query("SELECT RGB(N,0,0),QBColor(N) FROM T", Default::default()).unwrap();
    prepare(parsed.query(), &MetadataOnly(self::source())).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let checkpoints = Cell::new(0);
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&|| {
                checkpoints.set(checkpoints.get() + 1);
                false
            }),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=checkpoints.get() {
        let calls = Cell::new(0);
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                cancelled: Some(&|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }),
                ..Default::default()
            },
        );
        assert!(matches!(
            result,
            Err(QueryError::Evaluation(EvalError::Cancelled)) | Err(QueryError::Cancelled)
        ));
    }
}

#[test]
fn palette_parameter_overflow_is_distinct_from_an_in_range_integer_outside_the_palette() {
    let source = source();
    let parsed = parse_query("PARAMETERS P LONG; SELECT QBColor(P)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (value, error) in [
        (32767, EvalError::Domain),
        (32768, EvalError::Overflow),
        (-32768, EvalError::Domain),
        (-32769, EvalError::Overflow),
    ] {
        assert_eq!(
            plan.execute(&source, &[("P", Value::Int32(value))], Default::default())
                .err(),
            Some(QueryError::Evaluation(error))
        );
    }
    assert_eq!(
        plan.execute(&source, &[("P", Value::Int32(4))], Default::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(128)]]
    );
}
