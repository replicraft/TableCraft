//! Explicit Text grouping finishes all membership before aggregate consumers.
use super::*;
use std::cmp::Ordering;
use tablecraft_expressions::{EvalError, comparison::Collation};

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn source() -> Rows {
    let mut source = Rows::new([]);
    source.columns = vec![
        Column::new("K".into(), ScalarType::Text, false).unwrap(),
        Column::new("N".into(), ScalarType::Int32, false).unwrap(),
    ];
    source.rows = [("a", 1), ("b", 5), ("a", 2)]
        .into_iter()
        .map(|(key, number)| vec![Value::Text(key.into()), Value::Int32(number)])
        .collect();
    source
}

#[test]
fn text_key_aliases_and_all_current_aggregates_match_small_buffered_models() {
    for sql in [
        "SELECT K AS Label,Label,Len(K),Count(*) AS C,C FROM T GROUP BY K",
        "SELECT K,Count(*),Sum(N),Avg(N),Min(N),Max(N) FROM T GROUP BY K HAVING Count(*)>1",
        "SELECT K FROM T GROUP BY K",
    ] {
        for declared_integral in [false, true] {
            let source = source();
            let syntax = parse_query(sql, Default::default()).unwrap();
            let prepared = prepare(syntax.query(), &source).unwrap();
            let options = ExecutionOptions {
                declared_integral,
                collation: Some(&Ordinal),
                ..Default::default()
            };
            let buffered = prepared.execute(&source, &[], options).unwrap();
            let mut output = Vec::new();
            source.visits.set(0);
            let summary = prepared
                .execute_progressive(&source, &[], options, &mut |row| {
                    assert_eq!(source.visits.get(), source.rows.len());
                    output.push(row.to_vec());
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(output, buffered.rows(), "{sql}");
            assert_eq!(summary.scanned_rows, 3);
        }
    }
}

#[test]
fn text_group_eof_failure_precedes_deferred_aggregate_operands() {
    let mut source = source();
    source.eof_error = true;
    let syntax = parse_query("SELECT K,Sum(1/0) FROM T GROUP BY K", Default::default()).unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let mut emitted = 0;
    let result = prepared.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&Ordinal),
            ..Default::default()
        },
        &mut |_| {
            emitted += 1;
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
    assert_eq!(emitted, 0);
}

#[test]
fn final_top_omissions_do_not_hide_accepted_text_group_projection_errors() {
    let source = source();
    let syntax = parse_query(
        "SELECT TOP 1 K,1/(Count(*)-1) FROM T GROUP BY K HAVING True",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    let result = prepared.execute_progressive(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&Ordinal),
            ..Default::default()
        },
        &mut |row| {
            output.push(row.to_vec());
            Ok::<_, ()>(())
        },
    );
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    assert_eq!(output.len(), 1);
    assert_eq!(output[0][0], Value::Text("a".into()));
}
