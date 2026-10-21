use super::*;

#[test]
fn hidden_membership_filters_displayed_children_without_multiplying_them() {
    let source = Collections::new();
    let query = parse_query(
        "SELECT Id,Tags.Value FROM Items WHERE Other.Value=5",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert!(source.requests.borrow().is_empty());
    assert_eq!(source.ordinary.get(), 0);
    let expected = [
        vec![Value::Int32(1), Value::Int32(7)],
        vec![Value::Int32(1), Value::Int32(8)],
        vec![Value::Int32(1), Value::Int32(8)],
    ];
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        expected
    );
    assert_eq!(*source.requests.borrow(), [(vec![1], vec![0, 1])]);
    let sql = "SELECT D.Id,D.V FROM (SELECT Id,Tags.Value AS V FROM Items WHERE Other.Value IN (4,5)) AS D";
    assert_eq!(run(sql, &source).unwrap().rows(), expected);
    let sql = "SELECT Tags.Value AS V,Count(*) FROM Items WHERE Other.Value=5 GROUP BY Tags.Value ORDER BY V";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Int32(7), Value::Int64(1)],
            vec![Value::Int32(8), Value::Int64(2)]
        ]
    );
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Other.Value=6",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(2), Value::Null]]
    );
    // Unlike displayed empty collections, empty hidden lists have no NULL item.
    assert!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Other.Value IS NULL",
            &source
        )
        .unwrap()
        .rows()
        .is_empty()
    );
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE NOT Other.Value=5 ORDER BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(2), Value::Null],
            vec![Value::Int32(3), Value::Null]
        ]
    );
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Tags.Value=8 AND Other.Value=4",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        2
    );
}

#[test]
fn several_displayed_collections_expand_independently_of_hidden_item_count() {
    let mut source = Collections::new();
    source
        .values
        .push(Column::new("Third".into(), ScalarType::Int32, true).unwrap());
    source
        .unavailable
        .push(SchemaName::new("Third".into()).unwrap());
    for (i, lists) in source.lists.iter_mut().enumerate() {
        lists.push(if i == 0 {
            vec![Value::Int32(9), Value::Int32(9)]
        } else {
            vec![]
        });
    }
    assert_eq!(
        run(
            "SELECT Tags.Value,Other.Value FROM Items WHERE Third.Value=9",
            &source
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(7), Value::Int32(4)],
            vec![Value::Int32(7), Value::Int32(5)],
            vec![Value::Int32(8), Value::Int32(4)],
            vec![Value::Int32(8), Value::Int32(5)],
            vec![Value::Int32(8), Value::Int32(4)],
            vec![Value::Int32(8), Value::Int32(5)],
        ]
    );
}

#[test]
fn displayed_values_are_scalar_inputs_to_a_hidden_membership_atom() {
    let source = Collections::new();
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Other.Value=Tags.Value-3",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        3
    );
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Other.Value<>Tags.Value-3",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(3), Value::Null]]
    );
    let mut source = Collections::new();
    // Duplicate scalar parents must not cause list matching by scalar equality.
    source.rows[1] = source.rows[0].clone();
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Other.Value=6",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1), Value::Null]]
    );
}

#[test]
fn mixed_memberships_keep_join_aliases_outer_absence_and_derived_scopes() {
    let source = Collections::new();
    let sql = "SELECT A.Id,A.Tags.Value,B.Id FROM Items AS A,Items AS B WHERE B.Tags.Value=7 ORDER BY A.Id,A.Tags.Value";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 5);
    let sql = "SELECT A.Id,A.Tags.Value,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id WHERE B.Other.Value=6";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
    let sql = "SELECT A.Id,A.Tags.Value,B.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+1=B.Id WHERE A.Id=3 OR B.Other.Value=6 ORDER BY A.Id";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 4);
    let sql = "SELECT A.Id,A.Tags.Value FROM Items AS A LEFT JOIN Items AS B ON A.Id+10=B.Id WHERE B.Other.Value IS NULL";
    assert!(run(sql, &source).unwrap().rows().is_empty());
    let sql = "SELECT D.Id,A.Tags.Value FROM (SELECT Id FROM Items) AS D,Items AS A WHERE A.Other.Value=D.Id+3 ORDER BY D.Id";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 7);
    let sql = "SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT B.Tags.Value FROM Items AS B WHERE B.Other.Value=A.Id+3) ORDER BY A.Id";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
}

#[test]
fn mixed_scans_validate_hidden_and_displayed_values_before_top_or_false_filters() {
    let source = Collections::new();
    for sql in [
        "SELECT TOP 1 Tags.Value FROM Items WHERE 1 \\ (Other.Value-5)>0",
        "SELECT Tags.Value FROM Items WHERE False AND 1 \\ (Other.Value-5)>0",
        "SELECT Tags.Value FROM Items WHERE True OR 1 \\ (Other.Value-5)>0",
        "SELECT Tags.Value FROM Items WHERE Other.Value=4 AND 1 \\ (Tags.Value-8)>0",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(
                tablecraft_expressions::EvalError::DivisionByZero
            )),
            "{sql}"
        );
    }
    for member in [0, 1] {
        let mut source = Collections::new();
        source.ignore_errors = true;
        source.lists[2][member].push(Value::Text("bad".into()));
        assert!(matches!(
            run(
                "SELECT TOP 1 Tags.Value FROM Items WHERE False AND Other.Value=5",
                &source
            ),
            Err(QueryError::Schema(_))
        ));
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        failure_persistence: None,
        ..Default::default()
    })]
    #[test]
    fn mixed_rows_match_an_independent_parent_fold_and_child_expansion(
        parents in prop::collection::vec(
            (
                prop::collection::vec(prop::option::of(0_i32..4), 0..5),
                prop::collection::vec(prop::option::of(0_i32..4), 0..5),
            ),
            0..7,
        ),
        needle in 0_i32..4,
    ) {
        let mut source = Collections::new();
        source.rows = parents.iter()
            .map(|_| vec![Value::Int32(99), Value::Int32(1)])
            .collect();
        source.lists = parents.iter()
            .map(|(shown, hidden)| {
                [shown, hidden].map(|list| {
                    list.iter().map(|value| value.map_or(Value::Null, Value::Int32)).collect()
                }).to_vec()
            })
            .collect();
        let expected = parents.iter()
            .filter(|(_, hidden)| hidden.contains(&Some(needle)))
            .flat_map(|(shown, _)| {
                if shown.is_empty() {
                    vec![vec![Value::Null]]
                } else {
                    shown.iter().map(|value| vec![value.map_or(Value::Null, Value::Int32)]).collect()
                }
            })
            .collect::<Vec<_>>();
        let sql = format!("SELECT Tags.Value FROM Items WHERE Other.Value={needle}");
        let result = run(&sql, &source).unwrap();
        prop_assert_eq!(result.rows(), expected);
    }
}

#[test]
fn mixed_parameters_metadata_budgets_and_all_cancellation_checkpoints_are_enforced() {
    let source = Collections::new();
    let query = parse_query(
        "PARAMETERS Needle LONG; SELECT Tags.Value FROM Items WHERE Other.Value=Needle",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for (needle, count) in [(5, 3), (6, 1)] {
        assert_eq!(
            plan.execute(
                &source,
                &[("Needle", Value::Int32(needle))],
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
                &[("Needle", Value::Int32(5))],
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
            &[("Needle", Value::Int32(5))],
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
        assert_eq!(execute().err(), Some(QueryError::Cancelled));
    }
    source.changed.set(true);
    assert_eq!(
        plan.execute(
            &source,
            &[("Needle", Value::Int32(5))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::SchemaChanged)
    );
}
