//! Predicate suppression is a boundary before selected aggregate replay.
use super::*;
use tablecraft_core::values::Double;
use tablecraft_expressions::EvalError;

fn collect(sql: &str, source: &Rows) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut output = Vec::new();
    plan.execute_progressive(source, &[], Default::default(), &mut |row| {
        assert_eq!(source.visits.get(), source.rows.len());
        output.push(row.to_vec());
        Ok(())
    })?;
    Ok(output)
}

#[test]
fn rejected_global_groups_never_evaluate_selected_operands_or_raw_publication() {
    for predicate in ["False", "NULL", "Count(*)=0"] {
        let source = Rows::new([1, 2, 3]);
        let sql = format!(
            "SELECT Sum(1/0),Avg(1/0),Min(1/0),Max(1/0),Count(1/0),
                    ChrW(55296),1/0 FROM T HAVING {predicate}"
        );
        assert!(collect(&sql, &source).unwrap().is_empty());
        assert_eq!(source.scans.get(), 1);
    }
}

#[test]
fn accepted_groups_replay_only_their_original_rows_in_encounter_order() {
    let source = Rows::new([2, 1, 2, 3, 1]);
    let sql = "SELECT N,Sum(1/(N-2)),Count(*) AS C,C+CLngLng(1)
               FROM T GROUP BY N HAVING N<>2";
    assert_eq!(
        collect(sql, &source).unwrap(),
        [
            vec![
                Value::Int32(1),
                Value::Double(Double::from_float(-2.0)),
                Value::Int64(2),
                Value::Int64(3)
            ],
            vec![
                Value::Int32(3),
                Value::Double(Double::from_float(1.0)),
                Value::Int64(1),
                Value::Int64(2)
            ],
        ]
    );
}

#[test]
fn having_only_aggregates_and_empty_global_group_keep_their_own_slots() {
    let source = Rows::new([1, 1, 2]);
    assert_eq!(
        collect("SELECT N FROM T GROUP BY N HAVING Count(*)>1", &source).unwrap(),
        [vec![Value::Int32(1)]]
    );
    let source = Rows::new([]);
    assert_eq!(
        collect("SELECT Count(*) AS C,C FROM T HAVING Count(*)=0", &source).unwrap(),
        [vec![Value::Int64(0), Value::Int64(0)]]
    );
    assert!(
        collect("SELECT N FROM T GROUP BY N HAVING Count(*)=0", &source)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn top_counts_accepted_groups_but_does_not_hide_later_accepted_errors() {
    let source = Rows::new([0, 1, 2]);
    let syntax = parse_query(
        "SELECT TOP 1 N,Sum(1/(2-N)) FROM T GROUP BY N HAVING N>0",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
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
    assert_eq!(output[0][0], Value::Int32(1));
}

#[test]
fn failed_eof_precedes_having_and_deferred_projection_work() {
    let mut source = Rows::new([1, 2]);
    source.eof_error = true;
    let result = collect("SELECT Sum(1/0) FROM T HAVING Count(*)>0", &source);
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Source))
    ));
}

#[test]
fn count_only_having_avoids_replay_rows_but_operand_replay_is_budgeted() {
    let source = Rows::new(std::iter::repeat_n(1, 100_001));
    let syntax = parse_query(
        "SELECT Count(*) AS C,C FROM T HAVING Count(*)>0",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let options = ExecutionOptions {
        limits: tablecraft_query::QueryLimits {
            bytes: 8192,
            rows: 200_100,
            steps: 10_000_000,
        },
        ..Default::default()
    };
    let mut output = Vec::new();
    plan.execute_progressive(&source, &[], options, &mut |row| {
        output.push(row.to_vec());
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(output, [vec![Value::Int64(100_001), Value::Int64(100_001)]]);

    let source = Rows::new(std::iter::repeat_n(1, 1000));
    let syntax = parse_query("SELECT Sum(N) FROM T HAVING False", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut emitted = 0;
    let result = plan.execute_progressive(&source, &[], options, &mut |_| {
        emitted += 1;
        Ok::<_, ()>(())
    });
    assert!(matches!(
        result,
        Err(StreamError::Query(QueryError::Limit(_)))
    ));
    assert_eq!(emitted, 0);
    assert!(source.visits.get() < 1000);
}
