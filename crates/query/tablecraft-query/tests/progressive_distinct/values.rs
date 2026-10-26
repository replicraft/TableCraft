use super::*;

#[test]
fn integral_and_null_tuples_keep_first_encounter_order_with_buffered_parity() {
    for (kind, values) in [
        (
            ScalarType::Boolean,
            vec![Value::Boolean(true), Value::Boolean(false)],
        ),
        (ScalarType::Byte, vec![Value::Byte(255), Value::Byte(0)]),
        (
            ScalarType::Int16,
            vec![Value::Int16(-32768), Value::Int16(32767)],
        ),
        (
            ScalarType::Int32,
            vec![Value::Int32(i32::MIN), Value::Int32(i32::MAX)],
        ),
        (
            ScalarType::Int64,
            vec![Value::Int64(i64::MAX), Value::Int64(i64::MIN)],
        ),
    ] {
        let input = vec![
            values[0].clone(),
            Value::Null,
            values[0].clone(),
            values[1].clone(),
            Value::Null,
        ];
        let expected = vec![
            vec![values[0].clone(), Value::Null],
            vec![Value::Null, Value::Null],
            vec![values[1].clone(), Value::Null],
        ];
        let source = Data::new(kind, input);
        let sql = "SELECT DISTINCT N,NULL AS Z FROM T";
        for declared in [false, true] {
            let options = ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            };
            assert_eq!(buffered(&source, sql, options), expected);
            assert_eq!(stream(&source, sql, options).unwrap(), expected);
        }
    }
}

#[test]
fn decimal_equivalence_preserves_first_scale_and_currency_extremes() {
    let decimal = |coefficient, scale| Value::Decimal(Decimal::new(coefficient, scale).unwrap());
    let source = Data::new(
        ScalarType::Decimal,
        vec![
            decimal(100, 2),
            decimal(10, 1),
            decimal(-20, 1),
            decimal(0, 28),
            decimal(0, 0),
        ],
    );
    let expected = vec![
        vec![decimal(100, 2)],
        vec![decimal(-20, 1)],
        vec![decimal(0, 28)],
    ];
    let sql = "SELECT DISTINCT N FROM T";
    assert_eq!(
        buffered(&source, sql, ExecutionOptions::default()),
        expected
    );
    assert_eq!(
        stream(&source, sql, ExecutionOptions::default()).unwrap(),
        expected
    );

    let low = Value::Currency(Currency::from_scaled(i64::MIN));
    let high = Value::Currency(Currency::from_scaled(i64::MAX));
    let source = Data::new(
        ScalarType::Currency,
        vec![low.clone(), high.clone(), low.clone()],
    );
    assert_eq!(
        stream(&source, sql, ExecutionOptions::default()).unwrap(),
        [vec![low], vec![high]]
    );
}

#[test]
fn top_is_applied_to_unique_rows_after_full_projection_and_eof() {
    let source = Data::new(
        ScalarType::Int32,
        vec![
            Value::Int32(1),
            Value::Int32(1),
            Value::Int32(2),
            Value::Int32(3),
        ],
    );
    let sql = "SELECT DISTINCT TOP 2 N FROM T";
    let expected = vec![vec![Value::Int32(1)], vec![Value::Int32(2)]];
    assert_eq!(
        buffered(&source, sql, ExecutionOptions::default()),
        expected
    );
    assert_eq!(
        stream(&source, sql, ExecutionOptions::default()).unwrap(),
        expected
    );

    let error = stream(
        &source,
        "SELECT DISTINCT TOP 1 CLng(10\\(N-3)) FROM T",
        ExecutionOptions::default(),
    );
    assert!(matches!(
        error,
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
}

#[test]
fn arm_distinct_precedes_outer_union_double_promotion() {
    let source = Data::new(
        ScalarType::Int64,
        vec![
            Value::Int64(9_007_199_254_740_992),
            Value::Int64(9_007_199_254_740_993),
        ],
    );
    let sql = "SELECT DISTINCT N FROM T UNION ALL SELECT CDbl(0) FROM T WHERE N=9007199254740992";
    let wide = Value::Double(Double::from_float(9_007_199_254_740_992.0));
    let expected = vec![
        vec![wide.clone()],
        vec![wide],
        vec![Value::Double(Double::from_float(0.0))],
    ];
    assert_eq!(
        buffered(&source, sql, ExecutionOptions::default()),
        expected
    );
    assert_eq!(
        stream(&source, sql, ExecutionOptions::default()).unwrap(),
        expected
    );
}

#[test]
fn active_declared_profile_admission_does_not_invent_default_result_types() {
    let source = Data::new(
        ScalarType::Int32,
        vec![Value::Int32(1), Value::Int32(1), Value::Int32(2)],
    );
    let parsed = parse_query("SELECT DISTINCT N+CInt(1) FROM T", Default::default()).unwrap();
    for prepared_declared in [false, true] {
        let plan = prepare_with_options(
            parsed.query(),
            &source,
            PreparationOptions {
                declared_arithmetic: prepared_declared,
                ..Default::default()
            },
        )
        .unwrap();
        let before = source.scans.get();
        let rejected =
            plan.execute_progressive(&source, &[], ExecutionOptions::default(), &mut |_| {
                Ok::<_, ()>(())
            });
        assert!(matches!(
            rejected,
            Err(StreamError::Query(QueryError::Unsupported))
        ));
        assert_eq!(source.scans.get(), before);
        let mut rows = Vec::new();
        plan.execute_progressive(
            &source,
            &[],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
            &mut |row| {
                rows.push(row.to_vec());
                Ok::<_, ()>(())
            },
        )
        .unwrap();
        assert_eq!(rows, [vec![Value::Int32(2)], vec![Value::Int32(3)]]);
    }
}

#[test]
fn terminal_distinct_composes_grouped_and_stateful_inputs() {
    let source = Data::new(
        ScalarType::Int32,
        vec![
            Value::Int32(1),
            Value::Int32(1),
            Value::Int32(2),
            Value::Int32(2),
        ],
    );
    for sql in [
        "SELECT DISTINCT Count(*) FROM T GROUP BY N",
        "SELECT DISTINCT C FROM (SELECT Count(*) AS C FROM T GROUP BY N) Q",
        "SELECT DISTINCT Count(*) FROM T GROUP BY N HAVING Count(*)>1",
    ] {
        assert_eq!(
            buffered(&source, sql, ExecutionOptions::default()),
            [vec![Value::Int64(2)]]
        );
        assert_eq!(
            stream(&source, sql, ExecutionOptions::default()).unwrap(),
            [vec![Value::Int64(2)]],
            "{sql}"
        );
    }
}

#[test]
fn empty_global_and_null_only_distinct_populations_stay_separate() {
    let empty = Data::new(ScalarType::Int32, Vec::new());
    assert!(
        stream(
            &empty,
            "SELECT DISTINCT N FROM T",
            ExecutionOptions::default()
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        stream(
            &empty,
            "SELECT DISTINCT Count(*) FROM T",
            ExecutionOptions::default()
        )
        .unwrap(),
        [vec![Value::Int64(0)]]
    );
    let nonempty = Data::new(ScalarType::Int32, vec![Value::Int32(1), Value::Int32(2)]);
    assert_eq!(
        stream(
            &nonempty,
            "SELECT DISTINCT NULL FROM T",
            ExecutionOptions::default()
        )
        .unwrap(),
        [vec![Value::Null]]
    );
}
