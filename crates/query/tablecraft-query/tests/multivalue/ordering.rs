use super::*;
use tablecraft_expressions::EvalError;
use tablecraft_query::QueryLimits;

#[test]
fn unprojected_sort_values_expand_duplicates_empty_lists_and_correlated_inputs() {
    let source = Collections::new();
    for sql in [
        "SELECT Id FROM Items ORDER BY Tags.Value,Id",
        "SELECT Id FROM Items ORDER BY (SELECT Tags.Value),Id",
        "SELECT D.Id FROM (SELECT Id FROM Items ORDER BY Tags.Value,Id) AS D",
    ] {
        assert_eq!(
            run(sql, &source).unwrap().rows(),
            &[
                vec![Value::Int32(2)],
                vec![Value::Int32(3)],
                vec![Value::Int32(1)],
                vec![Value::Int32(1)],
                vec![Value::Int32(1)],
            ],
            "{sql}"
        );
    }
    assert_eq!(
        run("SELECT TOP 1 Id FROM Items ORDER BY Tags.Value", &source)
            .unwrap()
            .rows(),
        &[vec![Value::Int32(2)], vec![Value::Int32(3)]]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Tags.Value=8 ORDER BY Tags.Value",
            &source
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int32(1)], vec![Value::Int32(1)]]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Other.Value=4 ORDER BY Tags.Value",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        3
    );
    // A different sort-only collection expands independently of the projection.
    assert_eq!(
        run(
            "SELECT Tags.Value FROM Items WHERE Id=1 ORDER BY Other.Value",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        6
    );
    assert_eq!(
        run("SELECT DISTINCT Id FROM Items ORDER BY Tags.Value", &source).err(),
        Some(QueryError::Unsupported)
    );
}

#[test]
fn hidden_having_and_aggregate_sort_inputs_expand_before_grouping() {
    let source = Collections::new();
    for sql in [
        "SELECT Id,Count(*) FROM Items GROUP BY Id HAVING Count(Tags.Value)>0",
        "SELECT Id,Count(*) FROM Items WHERE Other.Value=4 GROUP BY Id HAVING Count(Tags.Value)>0",
    ] {
        assert_eq!(
            run(sql, &source).unwrap().rows(),
            &[vec![Value::Int32(1), Value::Int64(3)]],
            "{sql}"
        );
    }
    assert!(
        run(
            "SELECT Id FROM Items WHERE Tags.Value=7 GROUP BY Id HAVING Count(Tags.Value)>2",
            &source
        )
        .unwrap()
        .rows()
        .is_empty()
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items GROUP BY Id ORDER BY Count(Tags.Value) DESC,Id",
            &source
        )
        .unwrap()
        .rows(),
        &[
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    assert_eq!(run("SELECT Id,Count(*) FROM Items GROUP BY Id HAVING Count(Tags.Value)>2 ORDER BY Count(Other.Value)", &source).unwrap().rows(), &[vec![Value::Int32(1), Value::Int64(6)]]);
    assert_eq!(
        run(
            "SELECT Count(*) FROM Items HAVING Count(Tags.Value)>0",
            &source
        )
        .unwrap()
        .rows(),
        &[vec![Value::Int64(5)]]
    );
    let result = run("TRANSFORM Count(*) SELECT Id FROM Items GROUP BY Id HAVING Count(Tags.Value)>0 ORDER BY Id PIVOT Id IN (1,2,3)", &source).unwrap();
    assert_eq!(
        result.rows(),
        &[vec![
            Value::Int32(1),
            Value::Int64(3),
            Value::Null,
            Value::Null
        ]]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items GROUP BY Id HAVING Tags.Value=7",
            &source
        )
        .err(),
        Some(QueryError::Grouping)
    );
}

#[test]
fn hidden_sort_and_having_preserve_join_aliases_and_outer_absence() {
    let source = Collections::new();
    assert_eq!(run("SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id ORDER BY B.Tags.Value,A.Id", &source).unwrap().rows(), &[
        vec![Value::Int32(1), Value::Int32(2)], vec![Value::Int32(2), Value::Int32(3)], vec![Value::Int32(3), Value::Null],
    ]);
    assert_eq!(run("SELECT A.Id,Count(*) FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id GROUP BY A.Id HAVING Count(B.Tags.Value)>0", &source).unwrap().rows(), &[vec![Value::Int32(1), Value::Int64(3)]]);
    assert_eq!(run("SELECT A.Id FROM Items AS A,Items AS B WHERE A.Id=1 AND B.Id=1 AND B.Tags.Value=7 ORDER BY A.Tags.Value", &source).unwrap().rows().len(), 3);
}

#[test]
fn hidden_sort_and_having_keep_late_errors_and_invalid_provider_lists() {
    let source = Collections::new();
    for sql in [
        "SELECT TOP 1 Id FROM Items ORDER BY 1/(Tags.Value-8)",
        "SELECT Id FROM Items WHERE Other.Value=4 GROUP BY Id HAVING Sum(1/(Tags.Value-8))>0",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
    for sql in [
        "SELECT TOP 1 Id FROM Items ORDER BY Tags.Value",
        "SELECT Id FROM Items WHERE False GROUP BY Id HAVING Count(Tags.Value)>0",
    ] {
        let mut source = Collections::new();
        source.ignore_errors = true;
        source.lists[2][0].push(Value::Text("invalid".into()));
        assert!(
            matches!(run(sql, &source), Err(QueryError::Schema(_))),
            "{sql}"
        );
    }
}

#[test]
fn hidden_sort_and_having_enforce_metadata_limits_rebinding_and_all_checkpoints() {
    let source = Collections::new();
    let parsed = parse_query("PARAMETERS p LONG; SELECT Id,Count(*) FROM Items WHERE Other.Value=p GROUP BY Id HAVING Count(Tags.Value)>0 ORDER BY Count(Tags.Value)", Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(source.requests.borrow().is_empty());
    for (parameter, count) in [(4, 1), (5, 1), (6, 0)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("p", Value::Int32(parameter))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows()
            .len(),
            count
        );
    }
    let calls = Cell::new(0);
    let checkpoint = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == checkpoint.get()
    };
    let execute = || {
        plan.execute(
            &source,
            &[("p", Value::Int32(4))],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    execute().unwrap();
    for position in 1..=calls.get() {
        calls.set(0);
        checkpoint.set(position);
        assert!(matches!(
            execute(),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    for limits in [
        QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        QueryLimits {
            rows: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &source,
                &[("p", Value::Int32(4))],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
    source.changed.set(true);
    assert_eq!(
        plan.execute(
            &source,
            &[("p", Value::Int32(4))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::SchemaChanged)
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x484156494e47534f),
        ..Default::default()
    })]
    #[test]
    fn hidden_group_inputs_match_independent_nullable_child_products(
        parents in prop::collection::vec((
            prop::collection::vec(prop::option::of(-3_i32..4), 0..5),
            prop::collection::vec(prop::option::of(-3_i32..4), 0..5),
        ), 0..7),
        threshold in 0_usize..4,
    ) {
        let mut source = Collections::new();
        source.rows = parents.iter().enumerate().map(|(index, _)| vec![Value::Int32(99), Value::Int32(index as i32 + 1)]).collect();
        source.lists = parents.iter().map(|(left, right)| {
            [left, right].map(|values| values.iter().map(|value| value.map_or(Value::Null, Value::Int32)).collect()).to_vec()
        }).collect();
        let mut groups = parents.iter().enumerate().filter_map(|(index, (left, right))| {
            let left_width = left.len().max(1);
            let right_width = right.len().max(1);
            let present = left.iter().filter(|value| value.is_some()).count() * right_width;
            let sort = right.iter().filter(|value| value.is_some()).count() * left_width;
            (present > threshold).then_some((index as i32 + 1, left_width * right_width, sort))
        }).collect::<Vec<_>>();
        groups.sort_by_key(|(id, _, count)| (std::cmp::Reverse(*count), *id));
        let expected = groups.into_iter().map(|(id, total, _)| vec![Value::Int32(id), Value::Int64(total as i64)]).collect::<Vec<_>>();
        let sql = format!("SELECT Id,Count(*) FROM Items GROUP BY Id HAVING Count(Tags.Value)>{threshold} ORDER BY Count(Other.Value) DESC,Id");
        let result = run(&sql, &source).unwrap();
        prop_assert_eq!(result.rows(), expected);
    }
}
