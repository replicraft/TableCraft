use super::*;

#[test]
fn prior_alias_chains_keep_values_and_paired_declared_metadata() {
    let source = Records::new();
    let syntax = parse_query(
        "SELECT CInt(1)+CInt(2) AS Seed,Seed AS Copy,Copy+CInt(1) AS NextValue",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(plan.output_types().unwrap(), [OutputType::Dynamic; 3]);
    for declared in [false, true] {
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: declared,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            [vec![Value::Int16(3), Value::Int16(3), Value::Int16(4)]]
        );
        for output in result.output_types() {
            assert_eq!(kind(*output), declared.then_some(ScalarType::Int16));
        }
    }
}

#[test]
fn parameter_aliases_reuse_a_prepared_plan_after_scalar_overflow() {
    let source = Records::new();
    let syntax = parse_query(
        "PARAMETERS P Short; SELECT P AS Seed,Seed+CInt(1) AS NextValue",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    for (input, expected) in [(1, Some(2)), (i16::MAX, None), (-2, Some(-1))] {
        let result = plan.execute(
            &source,
            &[("P", Value::Int16(input))],
            ExecutionOptions {
                declared_integral: true,
                ..Default::default()
            },
        );
        if let Some(expected) = expected {
            assert_eq!(
                result.unwrap().rows(),
                [vec![Value::Int16(input), Value::Int16(expected)]]
            );
        } else {
            assert_eq!(
                result.err(),
                Some(QueryError::Evaluation(EvalError::Overflow))
            );
        }
    }
}

#[test]
fn correlated_scalar_subquery_aliases_feed_later_arithmetic() {
    let source = Records::new();
    let result = run(
        "SELECT N,(SELECT N+CLng(10)) AS Prior,Prior+CLng(1) AS NextValue FROM T",
        &source,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), Value::Int32(11), Value::Int32(12)],
            vec![Value::Int32(2), Value::Int32(12), Value::Int32(13)],
            vec![Value::Int32(3), Value::Int32(13), Value::Int32(14)],
        ]
    );
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn aliases_preserve_null_and_error_identity_until_the_owning_consumer() {
    let source = Records::new();
    let nulls = run(
        "SELECT NULL AS Gone,IsNull(Gone) AS NullIdentity",
        &source,
        Default::default(),
    )
    .unwrap();
    assert_eq!(nulls.rows(), [vec![Value::Null, Value::Boolean(true)]]);
    assert_eq!(nulls.output_types()[0], OutputType::Null);
    let errors = run(
        "SELECT ErrorCode,Kind FROM (SELECT CVErr(7) AS Problem,CLng(Problem) AS ErrorCode,VarType(Problem) AS Kind) Q",
        &source,
        Default::default(),
    ).unwrap();
    assert_eq!(errors.rows(), [vec![Value::Int32(7), Value::Int16(10)]]);
}

#[test]
fn correlated_prior_date_alias_subtraction_retains_intervals_and_no_match_null() {
    let source = Records::new();
    let dates =
        "SELECT #2020-01-01# AS AtDate UNION ALL SELECT #2020-01-03# UNION ALL SELECT #2020-01-10#";
    let sql = format!(
        "SELECT O.AtDate,(SELECT Max(I.AtDate) FROM ({dates}) I WHERE I.AtDate<O.AtDate) AS PriorDate,O.AtDate-PriorDate AS Gap FROM ({dates}) O ORDER BY O.AtDate"
    );
    for declared in [false, true] {
        let result = run(
            &sql,
            &source,
            ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            },
        )
        .unwrap();
        let gaps = result
            .rows()
            .iter()
            .map(|row| row[2].clone())
            .collect::<Vec<_>>();
        assert_eq!(
            gaps,
            [
                Value::Null,
                Value::Double(Double::from_float(2.0)),
                Value::Double(Double::from_float(7.0)),
            ]
        );
        assert_eq!(result.rows()[0][1], Value::Null);
        assert!(
            result.rows()[1..]
                .iter()
                .all(|row| matches!(row[1], Value::DateTime(_)))
        );
        assert_eq!(kind(result.output_types()[2]), Some(ScalarType::Double));
    }
    assert_eq!(source.scans.get(), 0);
}
