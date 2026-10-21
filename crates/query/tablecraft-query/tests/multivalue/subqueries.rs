use super::*;
use tablecraft_expressions::EvalError;

#[test]
fn scalar_in_and_quantified_subqueries_fold_without_child_multiplication() {
    let source = Collections::new();
    for predicate in [
        "Tags.Value=(SELECT 7)",
        "Tags.Value IN (SELECT 7 UNION ALL SELECT 8)",
        "Tags.Value=ANY(SELECT 7 UNION ALL SELECT 8)",
        "Tags.Value>ALL(SELECT 5)",
        "Tags.Value=7 AND EXISTS(SELECT 1)",
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate}");
        assert_eq!(run(&sql, &source).unwrap().rows(), [vec![Value::Int32(1)]]);
    }
    for predicate in [
        "Tags.Value<>(SELECT 7)",
        "Tags.Value NOT IN (SELECT 7 UNION ALL SELECT 8)",
        "Tags.Value NOT IN (SELECT Null)",
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate}");
        assert_eq!(run(&sql, &source).unwrap().rows(), [vec![Value::Int32(2)]]);
    }
    let sql = "SELECT Id FROM Items WHERE Tags.Value=ALL(SELECT 1 WHERE False) ORDER BY Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1)], vec![Value::Int32(3)]]
    );
    let sql = "SELECT Id FROM Items WHERE Tags.Value NOT IN (SELECT 1 WHERE False) ORDER BY Id";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
    for predicate in [
        "Tags.Value=(SELECT 1 WHERE False)",
        "Tags.Value=ANY(SELECT 1 WHERE False)",
    ] {
        assert!(
            run(&format!("SELECT Id FROM Items WHERE {predicate}"), &source)
                .unwrap()
                .rows()
                .is_empty()
        );
    }
    let sql = "SELECT Id FROM Items WHERE EXISTS(SELECT 1 WHERE Tags.Value IS NULL)";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int32(3)]]);
}

#[test]
fn correlated_items_traverse_nested_scopes_without_becoming_output_aliases() {
    let source = Collections::new();
    for predicate in [
        "A.Tags.Value=(SELECT A.Id+6)",
        "(SELECT A.Tags.Value)=7",
        "(SELECT (SELECT A.Tags.Value))=8",
        "EXISTS(SELECT B.Id FROM Items AS B WHERE B.Id=A.Tags.Value-6)",
        "EXISTS(SELECT D.Id FROM (SELECT Id FROM Items) AS D WHERE D.Id=A.Tags.Value-6)",
        "EXISTS(SELECT B.Id FROM Items AS B WHERE B.Id=1 AND EXISTS(SELECT 1 WHERE A.Tags.Value=8))",
        "A.Tags.Value=(SELECT TOP 1 7 FROM Items AS B GROUP BY B.Id ORDER BY B.Id)",
        "A.Tags.Value=(SELECT 7 AS V UNION SELECT 7 ORDER BY V)",
    ] {
        let sql = format!("SELECT A.Id FROM Items AS A WHERE {predicate}");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            [vec![Value::Int32(1)]],
            "{sql}"
        );
    }
    let sql = "SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT B.Id FROM Items AS B WHERE B.Other.Value=A.Tags.Value-3)";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int32(1)]]);
    let sql = "SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT B.Id FROM Items AS B WHERE B.Other.Value=A.Tags.Value)";
    assert!(run(sql, &source).unwrap().rows().is_empty());
    let sql = "SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT B.Id FROM Items AS B INNER JOIN Items AS C ON B.Id=A.Tags.Value-6 WHERE C.Id=B.Id)";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int32(1)]]);
}

#[test]
fn mixed_joined_and_crosstab_criteria_keep_scalar_and_item_scopes_separate() {
    let source = Collections::new();
    let sql = "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=(SELECT B.Id+6) ORDER BY A.Id,B.Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [
            vec![Value::Int32(1), Value::Int32(1)],
            vec![Value::Int32(1), Value::Int32(2)]
        ]
    );
    let sql = "SELECT Id,Tags.Value FROM Items WHERE Other.Value=(SELECT Tags.Value-3)";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Tags.Value IN (SELECT 7) GROUP BY Id PIVOT Id";
    let result = run(sql, &source).unwrap();
    assert_eq!(result.columns(), ["Id", "1", "2", "3"]);
    assert_eq!(
        result.rows(),
        [vec![
            Value::Int32(1),
            Value::Int64(1),
            Value::Null,
            Value::Null
        ]]
    );
    let sql = "TRANSFORM Count(*) SELECT 1 AS R FROM Items WHERE Other.Value=(SELECT Tags.Value-3) GROUP BY 1 PIVOT Tags.Value";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int64(1), Value::Int64(1), Value::Int64(2)]]
    );
    let sql = "SELECT A.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+10=B.Id WHERE EXISTS(SELECT 1 WHERE B.Tags.Value IS NULL)";
    assert!(run(sql, &source).unwrap().rows().is_empty());
}

#[test]
fn subqueries_retain_eager_late_errors_cardinality_and_no_item_empty_behavior() {
    let source = Collections::new();
    for sql in [
        "SELECT TOP 1 Id FROM Items WHERE Tags.Value=(SELECT 1 \\ (Tags.Value-8))",
        "SELECT Id FROM Items WHERE False AND Tags.Value=(SELECT 1 \\ 0)",
        "SELECT Id FROM Items WHERE True OR Tags.Value IN (SELECT 7 UNION ALL SELECT 1 \\ 0)",
        "SELECT Id FROM Items WHERE Tags.Value=7 AND EXISTS(SELECT 1 \\ 0)",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
    let sql = "SELECT Id FROM Items WHERE Tags.Value=(SELECT 7 UNION ALL SELECT 8)";
    assert_eq!(run(sql, &source).err(), Some(QueryError::Cardinality));
    let mut source = Collections::new();
    for lists in &mut source.lists {
        lists[0].clear();
    }
    let sql = "SELECT Id FROM Items WHERE Tags.Value=(SELECT 1 \\ 0)";
    assert!(run(sql, &source).unwrap().rows().is_empty());
    let sql = "SELECT Id FROM Items WHERE Tags.Value=7 AND EXISTS(SELECT 1 \\ 0)";
    assert_eq!(
        run(sql, &source).err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}

proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn generated_subquery_membership_matches_independent_parent_selection(
        lists in prop::collection::vec(prop::collection::vec(prop::option::of(0_i32..4), 0..5), 0..8),
        needle in 0_i32..4,
    ) {
        let mut source = Collections::new();
        source.rows = lists.iter().enumerate()
            .map(|(id, _)| vec![Value::Int32(99), Value::Int32(id as i32)]).collect();
        source.lists = lists.iter().map(|list| vec![
            list.iter().map(|value| value.map_or(Value::Null, Value::Int32)).collect(), vec![],
        ]).collect();
        let expected = lists.iter().enumerate()
            .filter(|(_, list)| list.contains(&Some(needle)) || list.contains(&Some(needle + 1)))
            .map(|(id, _)| vec![Value::Int32(id as i32)]).collect::<Vec<_>>();
        let sql = format!("SELECT Id FROM Items WHERE Tags.Value IN (SELECT {needle} UNION ALL SELECT {needle}+1) ORDER BY Id");
        let result = run(&sql, &source).unwrap();
        prop_assert_eq!(result.rows(), expected);
    }
}

#[test]
fn subquery_plans_rebind_parameters_reject_metadata_changes_and_enforce_all_checkpoints() {
    let source = Collections::new();
    let query = parse_query("PARAMETERS Needle LONG; SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT B.Id FROM Items AS B WHERE B.Other.Value=A.Tags.Value-Needle)", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert!(source.requests.borrow().is_empty());
    assert_eq!(source.ordinary.get(), 0);
    for (needle, count) in [(3, 1), (10, 0)] {
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
                &[("Needle", Value::Int32(3))],
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
            &[("Needle", Value::Int32(3))],
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
            &[("Needle", Value::Int32(3))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::SchemaChanged)
    );
}
