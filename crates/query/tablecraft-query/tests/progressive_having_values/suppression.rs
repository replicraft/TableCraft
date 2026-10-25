use super::*;

#[test]
fn false_and_null_having_suppress_all_projected_semantic_and_publication_errors() {
    let source = Data::fixture();
    for predicate in ["False", "NULL"] {
        for projected in [
            "Sum(1/0)",
            "1/0,Count(*)",
            "ChrW(55296),Count(*)",
            "CVErr(5),Count(*)",
            "Sum('not numeric')",
        ] {
            let sql = format!("SELECT {projected} FROM T HAVING {predicate}");
            for declared in [false, true] {
                let before = source.visits.get();
                let actual = stream(
                    &source,
                    &sql,
                    ExecutionOptions {
                        declared_integral: declared,
                        ..Default::default()
                    },
                )
                .unwrap();
                assert!(actual.is_empty(), "{sql}");
                assert_eq!(source.visits.get() - before, source.rows.len());
                assert!(source.eof.get());

                let syntax = parse_query(&sql, Default::default()).unwrap();
                let buffered = prepare(syntax.query(), &source)
                    .unwrap()
                    .execute(&source, &[], options(declared))
                    .unwrap();
                assert!(buffered.rows().is_empty(), "{sql}");
            }
        }
    }
}

#[test]
fn rejected_group_does_not_replay_its_dividing_projection_operand() {
    let source = Data::fixture();
    let sql = "SELECT K,Sum(1/(K-2)) FROM T GROUP BY K HAVING K<>2";
    let expected = vec![
        vec![Value::Int32(1), Value::Double(Double::from_float(-2.0))],
        vec![Value::Int32(3), Value::Double(Double::from_float(1.0))],
    ];
    for declared in [false, true] {
        assert_eq!(stream(&source, sql, options(declared)).unwrap(), expected);
    }
}

#[test]
fn having_only_five_aggregate_states_filter_using_independent_integer_answers() {
    let source = Data::fixture();
    let sql = "SELECT K FROM T GROUP BY K HAVING Count(*)>=2 AND Sum(N)>3 \
               AND Avg(N)>1 AND Min(N)>=1 AND Max(N)<=5";
    // Key2 has values2,5: count2, sum7, mean3.5, min2 and max5.
    // Key1's1,2 sum to3; each remaining population has only one row.
    let expected = vec![vec![Value::Int32(2)]];
    for declared in [false, true] {
        assert_eq!(stream(&source, sql, options(declared)).unwrap(), expected);
    }
}

#[test]
fn empty_global_and_empty_keyed_having_keep_distinct_population_semantics() {
    let source = Data::new(&[]);
    for declared in [false, true] {
        let accepted = "SELECT CLng(7) FROM T HAVING Count(*)=0";
        assert_eq!(
            stream(&source, accepted, options(declared)).unwrap(),
            [vec![Value::Int32(7)]]
        );
        for sql in [
            "SELECT CLng(7) FROM T HAVING Count(*)>0",
            "SELECT Sum(N) FROM T HAVING Sum(N)>0",
            "SELECT K,Sum(1/0) FROM T GROUP BY K HAVING 1/0>0",
        ] {
            assert!(stream(&source, sql, options(declared)).unwrap().is_empty());
        }
        let accepted = "SELECT Sum(N) FROM T HAVING Sum(N) IS NULL";
        assert_eq!(
            stream(&source, accepted, options(declared)).unwrap(),
            [vec![Value::Null]]
        );
        let failing = "SELECT Count(*) FROM T HAVING 1/0>0";
        assert!(matches!(
            stream(&source, failing, options(declared)),
            Err(StreamError::Query(QueryError::Evaluation(
                EvalError::DivisionByZero
            )))
        ));
    }
}
