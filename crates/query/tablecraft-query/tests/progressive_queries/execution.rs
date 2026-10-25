use super::*;
use std::cmp::Ordering;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::source::CollationResolver;

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

#[test]
fn eligible_projections_and_typed_parameters_match_buffered_results() {
    for sql in [
        "SELECT N,N+1 FROM T WHERE N>=2",
        "PARAMETERS P Long; SELECT N+P FROM T WHERE N>P",
        "SELECT IIf(N>1,'yes','no') FROM T",
        "SELECT ChrW(65),Len(ChrW(55296)) FROM T",
    ] {
        let source = Rows::new([1, 2, 3]);
        let parsed = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let parameters = if sql.starts_with("PARAMETERS") {
            vec![("P", Value::Int32(1))]
        } else {
            vec![]
        };
        let options = ExecutionOptions {
            collation: Some(&Ordinal),
            ..Default::default()
        };
        let expected = plan.execute(&source, &parameters, options).unwrap();
        let mut actual = Vec::new();
        let summary = plan
            .execute_progressive(&source, &parameters, options, &mut |row| {
                actual.push(row.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(actual, expected.rows(), "{sql}");
        assert_eq!(summary.scanned_rows, 3, "{sql}");
        assert_eq!(summary.emitted_rows, actual.len() as u64, "{sql}");
    }
}

#[test]
fn eager_projection_failure_never_emits_a_partial_row() {
    let source = Rows::new([1, 0, 2]);
    let parsed = parse_query("SELECT N,IIf(True,1,1/N) FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut output = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        output.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(output.len(), 1);
    assert_eq!(source.visits.get(), 2);
}

#[test]
fn invalid_raw_output_is_refused_before_that_row_reaches_the_sink() {
    for expression in ["ChrW(55296)", "ChrB(65)"] {
        let source = Rows::new([1]);
        let sql = format!("SELECT N,{expression} FROM T");
        let parsed = parse_query(&sql, Default::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        let calls = Cell::new(0);
        let result = plan.execute_progressive(&source, &[], Default::default(), &mut |_| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(())
        });
        assert!(
            matches!(
                result,
                Err(StreamError::Query(QueryError::Evaluation(
                    EvalError::UnrepresentableText
                )))
            ),
            "{sql}"
        );
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn first_sink_failure_survives_a_source_that_swallows_callback_failures() {
    let mut source = Rows::new([1, 2, 3]);
    source.swallow = true;
    let parsed = parse_query("SELECT N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut delivered = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        delivered.push(row.to_vec());
        if delivered.len() == 2 {
            Err("first sink error")
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(StreamError::Sink("first sink error"))));
    assert_eq!(
        delivered,
        vec![vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
}

struct Resolver {
    requested: Cell<usize>,
}

impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert_eq!(policy.host_key(), 7);
        self.requested.set(self.requested.get() + 1);
        Ok(&Ordinal)
    }
}

#[test]
fn projected_text_uses_the_declared_field_policy_and_rechecks_metadata() {
    let mut source = Rows::new([]);
    source.columns = vec![Column::new("N".into(), ScalarType::Text, false).unwrap()];
    source.rows = vec![vec![Value::Text("a".into())], vec![Value::Text("b".into())]];
    source.policies = vec![Some(TextCollationId::from_host(7))];
    let parsed = parse_query("SELECT N FROM T WHERE N='a'", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let resolver = Resolver {
        requested: Cell::new(0),
    };
    let mut output = Vec::new();
    plan.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
        &mut |row| {
            output.push(row.to_vec());
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    assert_eq!(output, vec![vec![Value::Text("a".into())]]);
    assert!(resolver.requested.get() > 0);

    source.policies[0] = Some(TextCollationId::from_host(8));
    source.scans.set(0);
    let result =
        plan.execute_progressive(&source, &[], Default::default(), &mut |_| Ok::<_, ()>(()));
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::SchemaChanged))
    ));
    assert_eq!(source.scans.get(), 0);
}

#[test]
fn eof_source_failure_retains_accepted_prefix_without_a_success_summary() {
    let mut source = Rows::new([1, 2]);
    source.eof_error = true;
    let parsed = parse_query("SELECT N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let mut output = Vec::new();
    let result = plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        output.push(row.to_vec());
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(output, vec![vec![Value::Int32(1)], vec![Value::Int32(2)]]);
}

#[test]
fn inactive_single_table_distinctrow_preserves_duplicate_rows() {
    let source = Rows::new([1, 1, 2]);
    let parsed = parse_query("SELECT DISTINCTROW N FROM T", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let buffered = plan.execute(&source, &[], Default::default()).unwrap();
    let mut output = Vec::new();
    let summary = plan
        .execute_progressive(&source, &[], Default::default(), &mut |row| {
            output.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();

    // DISTINCTROW has no active record-key filtering for this single-table plan.
    let expected = vec![
        vec![Value::Int32(1)],
        vec![Value::Int32(1)],
        vec![Value::Int32(2)],
    ];
    assert_eq!(buffered.rows(), expected);
    assert_eq!(output, expected);
    assert_eq!(summary.scanned_rows, 3);
    assert_eq!(summary.emitted_rows, 3);
}
