use super::*;

#[test]
fn uniform_nullable_integral_tuples_keep_first_encounter_values_and_counts() {
    for kind in [
        ScalarType::Boolean,
        ScalarType::Byte,
        ScalarType::Int16,
        ScalarType::Int32,
        ScalarType::Int64,
    ] {
        let (source, model) = population(kind);
        let groups = grouped(&model);
        let expected_groups = groups
            .iter()
            .map(|&(first, second, count)| {
                vec![
                    encode(kind, first),
                    encode(ScalarType::Int16, second),
                    Value::Int64(count),
                ]
            })
            .collect::<Vec<_>>();
        let expected_distinct = groups
            .iter()
            .map(|&(first, second, _)| vec![encode(kind, first), encode(ScalarType::Int16, second)])
            .collect::<Vec<_>>();
        for declared in [false, true] {
            let options = ExecutionOptions {
                declared_integral: declared,
                ..Default::default()
            };
            let actual = run("SELECT K,G,Count(*) FROM T GROUP BY K,G", &source, options).unwrap();
            assert_eq!(actual, expected_groups, "{kind:?}");
            let actual = run("SELECT DISTINCT K,G FROM T", &source, options).unwrap();
            assert_eq!(actual, expected_distinct, "{kind:?}");
        }
    }
}

#[test]
fn twelve_thousand_unique_keys_fit_a_finite_work_budget_without_changing_order() {
    let source = Unique::new(12_000);
    for declared in [false, true] {
        let options = ExecutionOptions {
            declared_integral: declared,
            limits: QueryLimits {
                bytes: 128 * 1024 * 1024,
                rows: 200_000,
                steps: 10_000_000,
            },
            ..Default::default()
        };
        let groups = run("SELECT K,Count(*) FROM T GROUP BY K", &source, options).unwrap();
        let expected = (0..12_000)
            .rev()
            .map(|value| vec![Value::Int32(value), Value::Int64(1)])
            .collect::<Vec<_>>();
        assert_eq!(groups, expected);

        let distinct = run("SELECT DISTINCT K FROM T", &source, options).unwrap();
        let expected = (0..12_000)
            .rev()
            .map(|value| vec![Value::Int32(value)])
            .collect::<Vec<_>>();
        assert_eq!(distinct, expected);
    }
    assert_eq!(source.visits.get(), 48_000);
}

#[test]
fn known_null_keys_group_together_and_do_not_alias_numeric_zero() {
    let (source, _) = population(ScalarType::Int32);
    for sql in [
        "SELECT K,Count(*) FROM (SELECT NULL AS K FROM T) Q GROUP BY K",
        "SELECT K,Count(*) FROM (SELECT NULL AS K FROM T) Q GROUP BY K,K",
    ] {
        let actual = run(sql, &source, Default::default()).unwrap();
        assert_eq!(actual, [vec![Value::Null, Value::Int64(101)]]);
    }
    let actual = run("SELECT DISTINCT NULL FROM T", &source, Default::default()).unwrap();
    assert_eq!(actual, [vec![Value::Null]]);

    let source = MemorySource::default();
    let sql = "SELECT K,Count(*) FROM (SELECT CLng(0) AS K UNION ALL SELECT NULL UNION ALL SELECT CLng(0) UNION ALL SELECT NULL) Q GROUP BY K";
    let actual = run(sql, &source, Default::default()).unwrap();
    assert_eq!(
        actual,
        [
            vec![Value::Int32(0), Value::Int64(2)],
            vec![Value::Null, Value::Int64(2)]
        ]
    );
}

#[test]
fn union_distinct_stages_and_all_boundaries_preserve_written_order() {
    let source = MemorySource::default();
    for (sql, expected) in [
        (
            "SELECT CLng(2) AS K UNION SELECT CLng(1) UNION ALL SELECT CLng(2)",
            vec![2, 1, 2],
        ),
        (
            "SELECT CLng(2) AS K UNION ALL SELECT CLng(1) UNION SELECT CLng(2)",
            vec![2, 1],
        ),
        (
            "SELECT CLng(2) AS K UNION SELECT CLng(2) UNION ALL SELECT CLng(1) UNION ALL SELECT CLng(2)",
            vec![2, 1, 2],
        ),
    ] {
        for declared in [false, true] {
            let actual = run(
                sql,
                &source,
                ExecutionOptions {
                    declared_integral: declared,
                    ..Default::default()
                },
            )
            .unwrap();
            let expected = expected
                .iter()
                .map(|&value| vec![Value::Int32(value)])
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{sql}");
        }
    }
}

#[test]
fn dynamic_mixed_integral_results_keep_the_general_comparers_first_representative() {
    for input in [[0, 1, 0, 2], [1, 0, 1, 2]] {
        let table = MemoryTable::new(
            "T".into(),
            vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            input
                .into_iter()
                .map(|value| vec![Value::Int32(value)])
                .collect(),
        )
        .unwrap();
        let source = MemorySource::new(vec![table]).unwrap();
        let first = if input[0] == 0 {
            Value::Int16(7)
        } else {
            Value::Int32(7)
        };
        let relation = "(SELECT IIf(N=0,CInt(7),CLng(7)) AS K FROM T) Q";
        let actual = run(
            &format!("SELECT K,Count(*) FROM {relation} GROUP BY K"),
            &source,
            Default::default(),
        )
        .unwrap();
        assert_eq!(actual, [vec![first.clone(), Value::Int64(4)]]);
        let actual = run(
            &format!("SELECT DISTINCT K FROM {relation}"),
            &source,
            Default::default(),
        )
        .unwrap();
        assert_eq!(actual, [vec![first]]);
    }
}

#[test]
fn active_distinctrow_keeps_complete_record_tuples_before_projecting_repeated_keys() {
    let (source, model) = population(ScalarType::Int32);
    let unique_records = grouped(&model);
    let mut expected = Vec::new();
    for &(first, _, _) in &unique_records {
        expected.push(vec![encode(ScalarType::Int32, first)]);
    }
    for declared in [false, true] {
        let actual = run(
            "SELECT DISTINCTROW L.K FROM T L,T R",
            &source,
            ExecutionOptions {
                declared_integral: declared,
                limits: QueryLimits {
                    bytes: 128 * 1024 * 1024,
                    rows: 200_000,
                    steps: 10_000_000,
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn execution_profile_proofs_survive_declared_width_derived_outputs() {
    let source = Unique::new(12_000);
    for sql in [
        "SELECT DISTINCT V FROM (SELECT CInt(K)+CInt(0) AS V FROM T) Q",
        "SELECT V,Count(*) FROM (SELECT CInt(K)+CInt(0) AS V FROM T) Q GROUP BY V",
    ] {
        let syntax = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(syntax.query(), &source).unwrap();
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    declared_integral: true,
                    limits: QueryLimits {
                        bytes: 128 * 1024 * 1024,
                        rows: 200_000,
                        steps: 10_000_000,
                    },
                    ..Default::default()
                },
            )
            .unwrap();
        let expected = (0..12_000)
            .rev()
            .map(|value| {
                let mut row = vec![Value::Int16(i16::try_from(value).unwrap())];
                if sql.contains("Count(*)") {
                    row.push(Value::Int64(1));
                }
                row
            })
            .collect::<Vec<_>>();
        assert_eq!(result.rows(), expected, "{sql}");
    }
}
