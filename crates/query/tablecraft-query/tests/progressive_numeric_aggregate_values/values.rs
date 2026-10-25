use super::*;

#[test]
fn supported_source_carriers_match_buffered_results_in_both_execution_profiles() {
    for (kind, values, sum_type, mean_type) in [
        (
            ScalarType::Boolean,
            vec![Value::Boolean(true), Value::Boolean(false)],
            20,
            5,
        ),
        (
            ScalarType::Byte,
            vec![Value::Byte(1), Value::Byte(3)],
            20,
            5,
        ),
        (
            ScalarType::Int16,
            vec![Value::Int16(1), Value::Int16(3)],
            20,
            5,
        ),
        (
            ScalarType::Int32,
            vec![Value::Int32(1), Value::Int32(3)],
            20,
            5,
        ),
        (
            ScalarType::Int64,
            vec![Value::Int64(1), Value::Int64(3)],
            20,
            5,
        ),
        (
            ScalarType::Single,
            vec![
                Value::Single(Single::from_float(1.0)),
                Value::Single(Single::from_float(3.0)),
            ],
            5,
            5,
        ),
        (ScalarType::Double, vec![double(1.0), double(3.0)], 5, 5),
        (
            ScalarType::Currency,
            vec![
                Value::Currency(Currency::from_scaled(10_000)),
                Value::Currency(Currency::from_scaled(30_000)),
            ],
            6,
            5,
        ),
        (
            ScalarType::Decimal,
            vec![decimal(1, 0), decimal(3, 0)],
            14,
            14,
        ),
    ] {
        let mut values = values;
        values.push(Value::Null);
        let source = Data::new(kind, values);
        let sql = "SELECT Sum(N),Avg(N),Count(N),VarType(Sum(N)),VarType(Avg(N)),Round(Sum(N),Count(NULL)) FROM T";
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        for declared in [false, true] {
            let options = ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            };
            let buffered = plan.execute(&source, &[], options).unwrap();
            let progressive = stream(sql, &source, options).unwrap();
            assert_eq!(progressive, buffered.rows(), "{kind:?}");
            assert_eq!(progressive[0][2], Value::Int64(2));
            assert_eq!(progressive[0][3], Value::Int16(sum_type));
            assert_eq!(progressive[0][4], Value::Int16(mean_type));
        }
    }
}

#[test]
fn exact_floating_cancellation_and_decimal_half_even_answers_are_independent() {
    let source = Data::new(
        ScalarType::Double,
        vec![double(1e16), double(1.0), double(-1e16)],
    );
    let rows = stream("SELECT Sum(N),Avg(N) FROM T", &source, Default::default()).unwrap();
    // Exact total is 1 and exact mean is 1/3, rounded once to binary64.
    assert_eq!(
        rows,
        [vec![
            double(1.0),
            double(f64::from_bits(0x3fd5_5555_5555_5555))
        ]]
    );

    for (values, expected_units) in [
        (vec![decimal(0, 28), decimal(1, 28)], [1, 0]),
        (vec![decimal(1, 28), decimal(2, 28)], [3, 2]),
    ] {
        let source = Data::new(ScalarType::Decimal, values);
        let rows = stream("SELECT Sum(N),Avg(N) FROM T", &source, Default::default()).unwrap();
        assert_eq!(rows.len(), 1);
        let units = rows[0]
            .iter()
            .map(|value| {
                let Value::Decimal(value) = value else {
                    panic!("expected Decimal carrier");
                };
                // A normalized zero may have a different representation. Compare
                // exact integer units, independently of production rescaling.
                value.coefficient() * 10_i128.pow(u32::from(28 - value.scale()))
            })
            .collect::<Vec<_>>();
        assert_eq!(units, expected_units);
    }

    let maximum = (1_i128 << 96) - 1;
    let source = Data::new(
        ScalarType::Decimal,
        vec![decimal(maximum, 0), decimal(-maximum, 0), decimal(1, 28)],
    );
    let rows = stream("SELECT Sum(N) FROM T", &source, Default::default()).unwrap();
    assert_eq!(rows, [vec![decimal(1, 28)]]);
}

#[test]
fn date_means_keep_raw_serials_and_empty_populations_keep_null() {
    let source = Data::new(
        ScalarType::DateTime,
        vec![
            Value::DateTime(LegacyDateTime::from_ole_days(-0.25).unwrap()),
            Value::DateTime(LegacyDateTime::from_ole_days(0.25).unwrap()),
            Value::Null,
        ],
    );
    let rows = stream("SELECT Avg(N),Count(N) FROM T", &source, Default::default()).unwrap();
    assert_eq!(rows, [vec![double(0.0), Value::Int64(2)]]);

    for source in [
        Data::new(ScalarType::Decimal, Vec::new()),
        Data::new(ScalarType::Decimal, vec![Value::Null]),
    ] {
        let rows = stream(
            "SELECT Sum(N),Avg(N),Count(*) FROM T",
            &source,
            Default::default(),
        )
        .unwrap();
        assert_eq!(
            rows,
            [vec![
                Value::Null,
                Value::Null,
                Value::Int64(source.count() as i64)
            ]]
        );
    }
}

#[test]
fn generated_large_input_needs_no_source_snapshot_and_keeps_alias_and_union_carriers() {
    let source = Data::generated(100_001);
    let rows = stream(
        "SELECT Sum(N) AS Total,Avg(N) AS Mean,Total/Count(*) AS Ratio FROM T",
        &source,
        ExecutionOptions {
            limits: QueryLimits {
                bytes: 64 * 1024 * 1024,
                rows: 300_000,
                steps: 20_000_000,
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        rows,
        [vec![
            Value::Int64(5_000_150_001),
            double(50_001.0),
            double(50_001.0)
        ]]
    );
    assert_eq!(*source.projections.borrow(), [vec![0]]);

    let source = Data::new(ScalarType::Int32, vec![Value::Int32(1), Value::Int32(3)]);
    let syntax = parse_query(
        "SELECT Sum(N) AS V FROM T UNION ALL SELECT Avg(N) FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    let mut output = Vec::new();
    plan.execute_progressive(&source, &[], Default::default(), &mut |row| {
        assert!(!source.active.get());
        output.push(row.to_vec());
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(output, [vec![double(4.0)], vec![double(2.0)]]);
}
