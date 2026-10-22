use std::cmp::Ordering;
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation, functions::math::random::RndState};
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![
                Column::new("N".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, true).unwrap(),
            ],
            vec![
                vec![Value::Int32(1), Value::Text("a".into())],
                vec![Value::Int32(2), Value::Text("bb".into())],
                vec![Value::Int32(1), Value::Text("a".into())],
                vec![Value::Int32(3), Value::Null],
            ],
        )
        .unwrap(),
    ])
    .unwrap()
}

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn run(sql: &str) -> Vec<Vec<Value>> {
    let source = source();
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            },
        )
        .unwrap()
        .rows()
        .to_vec()
}

#[test]
fn distinct_sorts_repeated_qualified_fields_and_computed_outputs() {
    assert_eq!(
        run("SELECT DISTINCT Len(Label) AS Width FROM T ORDER BY Len(Label)"),
        [
            vec![Value::Null],
            vec![Value::Int32(1)],
            vec![Value::Int32(2)]
        ]
    );
    assert_eq!(
        run("SELECT DISTINCT T.N AS V FROM T ORDER BY T.N DESC"),
        [
            vec![Value::Int32(3)],
            vec![Value::Int32(2)],
            vec![Value::Int32(1)]
        ]
    );
    assert_eq!(
        run("SELECT DISTINCT N+1 AS V FROM T ORDER BY N+1"),
        [
            vec![Value::Int64(2)],
            vec![Value::Int64(3)],
            vec![Value::Int64(4)]
        ]
    );
    assert_eq!(
        run("SELECT DISTINCT Count(*),Max(N) FROM T GROUP BY N ORDER BY Count(*),Max(N) DESC"),
        [
            vec![Value::Int64(1), Value::Int32(3)],
            vec![Value::Int64(1), Value::Int32(2)],
            vec![Value::Int64(2), Value::Int32(1)]
        ]
    );
    assert_eq!(
        run("SELECT DISTINCT Count(*)+1 FROM T GROUP BY N ORDER BY Count(*)+1 DESC"),
        [vec![Value::Int64(3)], vec![Value::Int64(2)]]
    );
}

struct MetadataOnly(MemorySource);

impl Source for MetadataOnly {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.0.columns(name)
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("projected sort binding must not scan records");
    }
}

#[test]
fn volatile_opaque_unselected_and_algebraically_different_sorts_are_not_reused() {
    let source = MetadataOnly(source());
    for sql in [
        "SELECT DISTINCT N FROM T ORDER BY N+1",
        "SELECT DISTINCT N+1 FROM T ORDER BY 1+N",
        "SELECT DISTINCT Rnd() FROM T ORDER BY Rnd()",
        "SELECT DISTINCT Now() FROM T ORDER BY Now()",
        "SELECT DISTINCT DCount('*','T') FROM T ORDER BY DCount('*','T')",
        "SELECT DISTINCT (SELECT 1) FROM T ORDER BY (SELECT 1)",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    let parsed = parse_query(
        "SELECT DISTINCT T.N FROM T ORDER BY T.N",
        ParseLimits::default(),
    )
    .unwrap();
    prepare(parsed.query(), &source).unwrap();
}

#[test]
fn distinctrow_and_top_keep_their_existing_record_and_tie_rules() {
    assert_eq!(
        run("SELECT DISTINCTROW A.N+1 FROM T A,T B WHERE A.N=B.N ORDER BY A.N+1"),
        [
            vec![Value::Int64(2)],
            vec![Value::Int64(3)],
            vec![Value::Int64(4)]
        ]
    );
    assert_eq!(
        run("SELECT DISTINCT TOP 2 N+1 FROM T ORDER BY N+1 DESC"),
        [vec![Value::Int64(4)], vec![Value::Int64(3)]]
    );
}

#[test]
fn explicit_random_alias_draws_once_per_input_and_sort_failures_remain_visible() {
    let source = source();
    let parsed = parse_query(
        "SELECT DISTINCT Rnd() AS R FROM T ORDER BY R",
        ParseLimits::default(),
    )
    .unwrap();
    let state = RndState::new(123).unwrap();
    let expected = RndState::new(123).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                random: Some(&state),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows().len(), 4);
    for _ in 0..4 {
        expected.apply(None).unwrap();
    }
    assert_eq!(state.apply(None).unwrap(), expected.apply(None).unwrap());

    let source = MemorySource::new(vec![
        MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(1)], vec![Value::Int32(0)]],
        )
        .unwrap(),
    ])
    .unwrap();
    let parsed = parse_query(
        "SELECT DISTINCT 1/N FROM T ORDER BY 1/N",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(parsed.query(), &source).unwrap().execute(
        &source,
        &[],
        ExecutionOptions::default(),
    );
    assert_eq!(
        result.err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}
