use super::*;

fn source(binary: bool, ignore: bool) -> Restricted {
    let (kind, empty, present) = if binary {
        (
            ScalarType::Binary,
            Value::Binary(vec![]),
            Value::Binary(vec![1]),
        )
    } else {
        (
            ScalarType::Text,
            Value::Text(String::new()),
            Value::Text("a".into()),
        )
    };
    Restricted {
        source: MemorySource::new(vec![table(
            "T",
            &[("X", kind), ("Y", ScalarType::Int32)],
            vec![
                vec![Value::Null, Value::Int32(1)],
                vec![empty, Value::Int32(2)],
                vec![present, Value::Int32(3)],
            ],
        )])
        .unwrap(),
        seen: Cell::new(None),
        scans: Cell::new(0),
        ignore,
    }
}

#[test]
fn null_string_hints_preserve_empty_values_and_residuals_without_collation() {
    for binary in [false, true] {
        for ignore in [false, true] {
            let source = source(binary, ignore);
            let sql = parse_query("SELECT Y FROM T WHERE X IS NULL", Default::default()).unwrap();
            let plan = prepare(sql.query(), &source).unwrap();
            assert_eq!(source.scans.get(), 0);
            assert_eq!(source.seen.get(), None);

            let result = plan.execute(&source, &[], Default::default()).unwrap();
            assert_eq!(result.rows(), [vec![Value::Int32(1)]]);
            assert_eq!(source.seen.get(), Some(ScanConstraint::Null { column: 0 }));
            let result = run("SELECT Y FROM T WHERE X IS NULL AND X IS NULL", &source).unwrap();
            assert_eq!(result.rows(), [vec![Value::Int32(1)]]);
        }
    }
}

#[test]
fn nonnull_and_partial_unsafe_string_filters_do_not_acquire_null_hints() {
    for binary in [false, true] {
        let source = source(binary, false);
        let result = run("SELECT Y FROM T WHERE X IS NOT NULL", &source).unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int32(2)], vec![Value::Int32(3)]]
        );
        assert_eq!(source.seen.get(), None);

        let result = run("SELECT Y FROM T WHERE X IS NULL OR Y=2", &source).unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int32(1)], vec![Value::Int32(2)]]
        );
        assert_eq!(source.seen.get(), None);
        assert_eq!(
            run("SELECT Y FROM T WHERE X IS NULL AND 1/0=0", &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero))
        );
        assert_eq!(source.seen.get(), None);
    }
}
