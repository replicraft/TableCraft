use super::*;
use tablecraft_expressions::EvalError;

#[test]
fn on_members_expand_without_projection_and_retain_child_duplicates() {
    let source = Collections::new();
    let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id+6 ORDER BY A.Id,B.Id";
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert!(source.requests.borrow().is_empty());
    assert_eq!(source.ordinary.get(), 0);
    let expected = [
        vec![Value::Int32(1), Value::Int32(1)],
        vec![Value::Int32(1), Value::Int32(2)],
        vec![Value::Int32(1), Value::Int32(2)],
    ];
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        expected
    );
    let projected = run(
        &sql.replace("SELECT A.Id,B.Id", "SELECT A.Id,B.Id,A.Tags.Value"),
        &source,
    )
    .unwrap();
    assert_eq!(
        projected
            .rows()
            .iter()
            .map(|row| row[..2].to_vec())
            .collect::<Vec<_>>(),
        expected
    );
    let sql =
        "SELECT DISTINCT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id+6";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 2);
    let sql = "SELECT A.Id,Count(*) FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id+6 GROUP BY A.Id HAVING Count(A.Tags.Value)>2";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), Value::Int64(3)]]
    );
    let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Tags.Value";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 5);
}

#[test]
fn outer_joins_match_expanded_children_before_null_extension_or_where() {
    let source = Collections::new();
    let sql = "SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Tags.Value=B.Id+6 ORDER BY A.Id,B.Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(1), Value::Int32(2)],
            vec![Value::Int32(1), Value::Int32(2)],
            vec![Value::Int32(2), Value::Null],
            vec![Value::Int32(3), Value::Null],
        ]
    );
    let sql = "SELECT A.Id,B.Id FROM Items AS A RIGHT JOIN Items AS B ON A.Tags.Value=B.Id+6 ORDER BY B.Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(1), Value::Int32(2)],
            vec![Value::Int32(1), Value::Int32(2)],
            vec![Value::Null, Value::Int32(3)],
        ]
    );
    let sql = "SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Tags.Value=B.Id+6 WHERE A.Tags.Value=8";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 2);
    let sql = "SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Tags.Value=B.Id+6 WHERE B.Id IS NULL ORDER BY A.Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Int32(2), Value::Null],
            vec![Value::Int32(3), Value::Null]
        ]
    );
    let sql = "SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Tags.Value=B.Id+6 WHERE A.Id=1 AND B.Id=99";
    assert!(run(sql, &source).unwrap().rows().is_empty());
    let sql = "SELECT A.Id,B.Id FROM (SELECT Id FROM Items WHERE False) AS A RIGHT JOIN Items AS B ON A.Id=B.Other.Value ORDER BY B.Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Null, Value::Int32(1)],
            vec![Value::Null, Value::Int32(1)],
            vec![Value::Null, Value::Int32(2)],
            vec![Value::Null, Value::Int32(3)]
        ]
    );
}

#[test]
fn on_collections_compose_with_independent_membership_nested_joins_and_subqueries() {
    let source = Collections::new();
    let base = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id+6";
    for (predicate, count) in [
        ("A.Other.Value=5", 3),
        ("B.Other.Value=6", 2),
        ("A.Other.Value=5 AND B.Other.Value=6", 2),
    ] {
        assert_eq!(
            run(&format!("{base} WHERE {predicate}"), &source)
                .unwrap()
                .rows()
                .len(),
            count
        );
    }
    let sql = "SELECT A.Id,B.Id FROM Items AS A INNER JOIN Items AS B ON EXISTS(SELECT 1 WHERE A.Tags.Value=B.Id+6)";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
    let sql = "SELECT A.Id,C.Id FROM (Items AS A INNER JOIN Items AS B ON A.Id=B.Id) INNER JOIN Items AS C ON A.Tags.Value=C.Id+6 AND B.Id=C.Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), Value::Int32(1)]]
    );
    let sql = "SELECT D.Id FROM (SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id+6) AS D";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
    let sql = "TRANSFORM Count(*) SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id+6 WHERE A.Other.Value=5 GROUP BY A.Id PIVOT B.Id IN (1,2,3)";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![
            Value::Int32(1),
            Value::Int64(1),
            Value::Int64(2),
            Value::Null
        ]]
    );
    let sql = "SELECT A.Id FROM Items AS A INNER JOIN Items AS B ON A.Tags.Value=B.Id+6 ORDER BY A.Tags.Value DESC";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
}

#[test]
fn late_join_errors_and_invalid_on_lists_are_not_hidden_by_top_or_boolean_operands() {
    let source = Collections::new();
    for condition in [
        "True OR 1 \\ (A.Tags.Value-8)>0",
        "False AND 1 \\ (A.Tags.Value-8)>0",
    ] {
        let sql = format!("SELECT TOP 1 A.Id FROM Items AS A INNER JOIN Items AS B ON {condition}");
        assert_eq!(
            run(&sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero))
        );
    }
    let mut source = Collections::new();
    source.ignore_errors = true;
    source.lists[2][0].push(Value::Text("wrong carrier".into()));
    let sql =
        "SELECT TOP 1 A.Id FROM Items AS A INNER JOIN Items AS B ON False AND A.Tags.Value=B.Id";
    assert!(matches!(run(sql, &source), Err(QueryError::Schema(_))));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        ..Default::default()
    })]
    #[test]
    fn generated_outer_child_join_matches_an_independent_nullable_expansion(
        lists in prop::collection::vec(
            prop::collection::vec(prop::option::of(0_i32..4), 0..5), 0..7,
        ),
    ) {
        let mut source = Collections::new();
        source.rows = lists.iter().enumerate()
            .map(|(id, _)| vec![Value::Int32(99), Value::Int32(id as i32)])
            .collect();
        source.lists = lists.iter().map(|list| vec![
            list.iter().map(|value| value.map_or(Value::Null, Value::Int32)).collect(),
            vec![],
        ]).collect();
        let parent_count = lists.len();
        let expected = lists.iter().enumerate().flat_map(|(id, list)| {
            let children = if list.is_empty() { vec![None] } else { list.clone() };
            children.into_iter().map(move |value| vec![
                Value::Int32(id as i32),
                value.filter(|value| (*value as usize) < parent_count)
                    .map_or(Value::Null, Value::Int32),
            ])
        }).collect::<Vec<_>>();
        let sql = "SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Tags.Value=B.Id";
        let result = run(sql, &source).unwrap();
        prop_assert_eq!(result.rows(), expected);
    }
}

#[test]
fn on_member_plans_rebind_parameters_check_schema_and_all_execution_checkpoints() {
    let source = Collections::new();
    let parsed = parse_query("PARAMETERS Delta LONG; SELECT A.Id,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Tags.Value=B.Id+Delta WHERE B.Other.Value=6",ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (delta, count) in [(6, 2), (5, 1)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("Delta", Value::Int32(delta))],
                ExecutionOptions::default()
            )
            .unwrap()
            .rows()
            .len(),
            count
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
    for checkpoint in 1..=calls.get() {
        calls.set(0);
        stop.set(checkpoint);
        assert!(matches!(
            execute().err(),
            Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
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
