//! Unsupported comparisons cannot become apparently successful index selections.
use super::*;

fn source(rows: Vec<Vec<Value>>) -> Restricted {
    Restricted {
        source: MemorySource::new(vec![table("T", &[("X", ScalarType::Binary)], rows)]).unwrap(),
        seen: Cell::new(None),
        scans: Cell::new(0),
        ignore: false,
    }
}

#[test]
fn nonnull_binary_predicates_never_hide_type_errors_behind_exact_scan_hints() {
    for predicate in ["X=p", "p=X", "X<p", "X=p AND 1/0=0"] {
        let query = parse_query(
            &format!("PARAMETERS p BINARY; SELECT X FROM T WHERE {predicate}"),
            Default::default(),
        )
        .unwrap();
        for stored in [vec![], vec![0], vec![1, 0], vec![255; 255], vec![255; 256]] {
            for parameter in [vec![], vec![0], vec![1, 0], vec![128, 0, 255]] {
                let source = source(vec![vec![Value::Binary(stored.clone())]]);
                let plan = prepare(query.query(), &source).unwrap();
                assert!(matches!(
                    plan.execute(
                        &source,
                        &[("p", Value::Binary(parameter))],
                        Default::default(),
                    ),
                    Err(QueryError::Evaluation(EvalError::TypeMismatch))
                ));
                assert_eq!(source.seen.get(), None);
                assert_eq!(source.scans.get(), 1);
            }
        }
    }
}

#[test]
fn binary_null_and_empty_inputs_keep_their_existing_residual_truth() {
    let query = parse_query(
        "PARAMETERS p BINARY; SELECT X FROM T WHERE X=p",
        Default::default(),
    )
    .unwrap();
    for rows in [vec![], vec![vec![Value::Null]]] {
        let source = source(rows);
        let plan = prepare(query.query(), &source).unwrap();
        for parameter in [Value::Null, Value::Binary(vec![1])] {
            let result = plan
                .execute(&source, &[("p", parameter)], Default::default())
                .unwrap();
            assert!(result.rows().is_empty());
            assert_eq!(source.seen.get(), None);
        }
    }

    let source = source(vec![vec![Value::Binary(vec![1])], vec![Value::Null]]);
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(&source, &[("p", Value::Null)], Default::default())
        .unwrap();
    assert!(result.rows().is_empty());
    assert_eq!(source.seen.get(), None);
}
