use super::*;

#[test]
fn joined_membership_retains_parent_pairs_duplicates_and_alias_scopes() {
    let mut source = Collections::new();
    let query = parse_query(
        "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=7 ORDER BY A.Id,B.Id",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.ordinary.get(), 0);
    assert!(source.requests.borrow().is_empty());
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(1), Value::Int32(2)],
            vec![Value::Int32(1), Value::Int32(3)]
        ]
    );
    for sql in [
        "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=8 AND B.Other.Value=6",
        "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id<B.Id WHERE A.Tags.Value=8 AND B.Other.Value=6",
    ] {
        assert_eq!(
            run(sql, &source).unwrap().rows(),
            [vec![Value::Int32(1), Value::Int32(2)]]
        );
    }
    source.rows.push(source.rows[0].clone());
    source.lists.push(source.lists[0].clone());
    assert_eq!(
        run(
            "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=8 AND B.Other.Value=6",
            &source
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(1), Value::Int32(2)],
            vec![Value::Int32(1), Value::Int32(2)]
        ]
    );
}

#[test]
fn item_predicates_can_use_other_join_scalars_and_derived_leaf_positions() {
    let source = Collections::new();
    for sql in [
        "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=B.Id+6 ORDER BY A.Id,B.Id",
        "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Id<=B.Id WHERE A.Tags.Value=B.Id+6 ORDER BY A.Id,B.Id",
    ] {
        assert_eq!(
            run(sql, &source).unwrap().rows(),
            [
                vec![Value::Int32(1), Value::Int32(1)],
                vec![Value::Int32(1), Value::Int32(2)]
            ]
        );
    }
    assert_eq!(run("SELECT D.Id,A.Id FROM (SELECT Id FROM Items) AS D,Items AS A WHERE A.Tags.Value=D.Id+6 ORDER BY D.Id", &source).unwrap().rows(), [vec![Value::Int32(1),Value::Int32(1)],vec![Value::Int32(2),Value::Int32(1)]]);
    assert_eq!(run("SELECT A.Id,Count(*) FROM Items AS A,Items AS B WHERE A.Tags.Value=B.Id+6 GROUP BY A.Id", &source).unwrap().rows(), [vec![Value::Int32(1),Value::Int64(2)]]);
    assert_eq!(run("SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE NOT A.Tags.Value=B.Id+6 ORDER BY A.Id,B.Id", &source).unwrap().rows(), [vec![Value::Int32(1),Value::Int32(3)],vec![Value::Int32(2),Value::Int32(1)],vec![Value::Int32(2),Value::Int32(2)],vec![Value::Int32(2),Value::Int32(3)]]);
    assert_eq!(run("SELECT A.Id,C.Id FROM (Items AS A INNER JOIN Items AS B ON A.Id=B.Id) INNER JOIN Items AS C ON B.Id=C.Id WHERE A.Tags.Value=8 AND C.Other.Value=5", &source).unwrap().rows(), [vec![Value::Int32(1),Value::Int32(1)]]);
    assert_eq!(run("SELECT D.Id FROM (SELECT A.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=B.Id+6) AS D", &source).unwrap().rows(), [vec![Value::Int32(1)],vec![Value::Int32(1)]]);
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT B.Id FROM Items AS B,Items AS C WHERE B.Tags.Value=A.Id+6 AND C.Id=B.Id) ORDER BY A.Id", &source).unwrap().rows(), [vec![Value::Int32(1)],vec![Value::Int32(2)]]);
}

#[test]
fn outer_joins_keep_absent_parents_distinct_from_empty_or_null_collections() {
    let source = Collections::new();
    // A has an actual parent in every row, so existing empty/NULL rules apply.
    assert_eq!(run("SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id WHERE NOT A.Tags.Value=7 ORDER BY A.Id", &source).unwrap().rows(), [vec![Value::Int32(2),Value::Int32(3)]]);
    // B is absent for A=3: membership atoms are UNKNOWN, including IS NULL,
    // rather than manufacturing an empty list or a stored NULL element.
    assert_eq!(run("SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id WHERE B.Tags.Value IS NULL ORDER BY A.Id", &source).unwrap().rows(), [vec![Value::Int32(2),Value::Int32(3)]]);
    assert_eq!(run("SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id WHERE NOT B.Tags.Value=7 ORDER BY A.Id", &source).unwrap().rows(), [vec![Value::Int32(1),Value::Int32(2)]]);
    assert_eq!(run("SELECT A.Id,B.Id FROM Items AS A RIGHT JOIN Items AS B ON A.Id+1=B.Id WHERE A.Id IS NULL OR A.Tags.Value=7 ORDER BY B.Id", &source).unwrap().rows(), [vec![Value::Null,Value::Int32(1)],vec![Value::Int32(1),Value::Int32(2)]]);
    assert_eq!(run("SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id WHERE A.Id=3 OR B.Tags.Value=7", &source).unwrap().rows(), [vec![Value::Int32(3),Value::Null]]);
    assert_eq!(run("SELECT A.Id,C.Id FROM (Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id) LEFT JOIN Items AS C ON B.Id+1=C.Id WHERE C.Tags.Value IS NULL OR A.Id=3 ORDER BY A.Id", &source).unwrap().rows(), [vec![Value::Int32(1),Value::Int32(3)],vec![Value::Int32(3),Value::Null]]);
}

#[test]
fn joined_membership_does_not_hide_later_failures_or_malformed_provider_lists() {
    let source = Collections::new();
    for sql in [
        "SELECT TOP 1 A.Id FROM Items AS A,Items AS B WHERE 1 \\ (A.Tags.Value-8)>0",
        "SELECT A.Id FROM Items AS A,Items AS B WHERE True OR 1 \\ (B.Tags.Value-8)>0",
        "SELECT A.Id FROM Items AS A,Items AS B WHERE False AND 1 \\ (A.Tags.Value-8)>0",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(
                tablecraft_expressions::EvalError::DivisionByZero
            )),
            "{sql}"
        );
    }
    let mut source = Collections::new();
    source.ignore_errors = true;
    source.lists[0][0].push(Value::Text("wrong carrier".into()));
    assert!(matches!(
        run(
            "SELECT A.Id FROM Items AS A,Items AS B WHERE False AND A.Tags.Value=7",
            &source
        ),
        Err(QueryError::Schema(_))
    ));
    let source = Collections::new();
    for sql in [
        "SELECT A.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=B.Tags.Value",
        "SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id WHERE B.Other.Value=6",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    assert_eq!(source.ordinary.get(), 0);
    assert!(source.requests.borrow().is_empty());
}

proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn joined_parent_pairs_match_an_independent_nullable_membership_product(values in prop::collection::vec(prop::collection::vec(prop::option::of(0_i32..4),0..5),0..6)) {
        let mut source = Collections::new();
        source.rows = values.iter().enumerate().map(|(i,_)|vec![Value::Int32(99),Value::Int32(i as i32)]).collect();
        source.lists = values.iter().map(|list|vec![list.iter().map(|value|value.map_or(Value::Null,Value::Int32)).collect(),vec![]]).collect();
        let expected = values.iter().enumerate().flat_map(|(a,list)| (0..values.len()).filter_map(move |b| list.contains(&Some(b as i32)).then_some(vec![Value::Int32(a as i32),Value::Int32(b as i32)]))).collect::<Vec<_>>();
        let result = run("SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=B.Id ORDER BY A.Id,B.Id", &source).unwrap();
        prop_assert_eq!(result.rows(), expected);
    }
}

#[test]
fn joined_parameters_limits_and_every_execution_cancellation_checkpoint_are_checked() {
    let source = Collections::new();
    let query = parse_query("PARAMETERS Delta LONG; SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=B.Id+Delta ORDER BY A.Id,B.Id", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (offset, expected) in [(6, 2), (7, 1)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("Delta", Value::Int32(offset))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows()
            .len(),
            expected
        );
    }
    for limits in [
        tablecraft_query::QueryLimits {
            bytes: 1,
            ..Default::default()
        },
        tablecraft_query::QueryLimits {
            rows: 1,
            ..Default::default()
        },
        tablecraft_query::QueryLimits {
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            plan.execute(
                &source,
                &[("Delta", Value::Int32(6))],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(QueryError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    let stop = Cell::new(usize::MAX);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == stop.get()
    };
    let execute = || {
        plan.execute(
            &source,
            &[("Delta", Value::Int32(6))],
            ExecutionOptions {
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        )
    };
    execute().unwrap();
    let total = calls.get();
    assert!(total > 10);
    for checkpoint in 1..=total {
        calls.set(0);
        stop.set(checkpoint);
        assert_eq!(execute().err(), Some(QueryError::Cancelled));
    }
    source.changed.set(true);
    assert_eq!(
        plan.execute(
            &source,
            &[("Delta", Value::Int32(6))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::SchemaChanged)
    );
}
