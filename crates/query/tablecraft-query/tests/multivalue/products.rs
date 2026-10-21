use super::*;
use tablecraft_expressions::EvalError;

#[test]
fn multi_member_atoms_fold_products_once_and_keep_negative_empty_and_null_rules() {
    let source = Collections::new();
    let sql = "SELECT Id FROM Items WHERE Tags.Value=Other.Value+3";
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert!(source.requests.borrow().is_empty());
    assert_eq!(source.ordinary.get(), 0);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)]]
    );
    let sql = "SELECT Id FROM Items WHERE Tags.Value<>Other.Value+3 ORDER BY Id";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int32(2)], vec![Value::Int32(3)]]
    );
    let sql = "SELECT Id FROM Items WHERE Tags.Value=Other.Value";
    assert!(run(sql, &source).unwrap().rows().is_empty());
    let mut source = Collections::new();
    source.lists[2][1] = vec![Value::Int32(1)];
    let sql = "SELECT Id FROM Items WHERE Tags.Value<>Other.Value+3 ORDER BY Id";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int32(2)]]);
    let sql = "SELECT Id FROM Items WHERE (Tags.Value+Other.Value) IS NULL";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int32(3)]]);
    let mut source = Collections::new();
    source.columns.clear();
    for row in &mut source.rows {
        row.clear();
    }
    let sql = "SELECT 1 FROM Items WHERE Tags.Value=Other.Value+3";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int64(1)]]);
}

#[test]
fn multiple_members_keep_join_alias_identity_and_missing_owner_unknown() {
    let source = Collections::new();
    let sql = "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=B.Other.Value+3";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int32(1), Value::Int32(1)]]
    );
    let sql = "SELECT A.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+10=B.Id WHERE A.Other.Value<>B.Tags.Value";
    assert!(run(sql, &source).unwrap().rows().is_empty());
    let sql = "SELECT A.Id FROM Items AS A LEFT JOIN Items AS B ON A.Id+10=B.Id WHERE A.Other.Value<>B.Tags.Value OR A.Id=3";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int32(3)]]);
    let sql = "SELECT D.Id FROM (SELECT Id FROM Items WHERE Tags.Value=Other.Value+3) AS D";
    assert_eq!(run(sql, &source).unwrap().rows(), [vec![Value::Int32(1)]]);
    let sql = "TRANSFORM Count(*) SELECT Id FROM Items WHERE Tags.Value=Other.Value+3 GROUP BY Id PIVOT Id";
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
}

#[test]
fn tuple_captures_remain_distinct_across_nested_local_single_and_product_membership() {
    let source = Collections::new();
    for predicate in [
        "(SELECT (SELECT A.Tags.Value+A.Other.Value))=12",
        "EXISTS(SELECT B.Id FROM Items AS B WHERE B.Other.Value=A.Tags.Value-A.Other.Value)",
        "EXISTS(SELECT B.Id FROM Items AS B WHERE EXISTS(SELECT 1 WHERE B.Tags.Value-B.Other.Value=A.Tags.Value-4))",
        "A.Tags.Value=(SELECT TOP 1 A.Other.Value+3 AS V FROM Items AS B ORDER BY V,B.Id)",
    ] {
        let sql = format!("SELECT A.Id FROM Items AS A WHERE {predicate}");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            [vec![Value::Int32(1)]],
            "{sql}"
        );
    }
    let mut source = Collections::new();
    source
        .values
        .push(Column::new("Third".into(), ScalarType::Int32, true).unwrap());
    source
        .unavailable
        .push(SchemaName::new("Third".into()).unwrap());
    for lists in &mut source.lists {
        lists.push(vec![Value::Int32(3)]);
    }
    let sql = "SELECT Id,Tags.Value FROM Items WHERE Other.Value+Third.Value=Tags.Value";
    assert_eq!(run(sql, &source).unwrap().rows().len(), 3);
    let sql = "TRANSFORM Count(*) SELECT 1 AS R FROM Items WHERE Other.Value+Third.Value=(SELECT Tags.Value) GROUP BY 1 PIVOT Tags.Value";
    assert_eq!(
        run(sql, &source).unwrap().rows(),
        [vec![Value::Int64(1), Value::Int64(1), Value::Int64(2)]]
    );
}

#[test]
fn later_product_errors_and_invalid_lists_are_eager_even_with_empty_other_groups() {
    let source = Collections::new();
    for sql in [
        "SELECT TOP 1 Id FROM Items WHERE 1 \\ (Tags.Value-Other.Value-2)>0",
        "SELECT Id FROM Items WHERE True OR 1 \\ (Tags.Value-Other.Value-2)>0",
        "SELECT Id FROM Items WHERE False AND 1 \\ (Tags.Value-Other.Value-2)>0",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
    let mut source = Collections::new();
    source.ignore_errors = true;
    source.lists[0][0].clear();
    source.lists[0][1].push(Value::Text("wrong carrier".into()));
    let sql = "SELECT TOP 1 Id FROM Items WHERE False AND Tags.Value=Other.Value";
    assert!(matches!(run(sql, &source), Err(QueryError::Schema(_))));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn generated_joined_products_match_independent_nullable_pairs_without_parent_key_matching(
        lists in prop::collection::vec((
            prop::collection::vec(prop::option::of(0_i32..4), 0..5),
            prop::collection::vec(prop::option::of(0_i32..4), 0..5),
        ), 0..6),
        negative in any::<bool>(),
    ) {
        let mut source = Collections::new();
        source.rows = lists.iter().enumerate()
            .map(|(id, _)| vec![Value::Int32(id as i32),Value::Int32(99)]).collect();
        source.lists = lists.iter().map(|(a,b)| [a,b].map(|list| {
            list.iter().map(|value|value.map_or(Value::Null,Value::Int32)).collect()
        }).to_vec()).collect();
        let mut expected = Vec::new();
        for (a,(left,_)) in lists.iter().enumerate() {
            for (b,(_,right)) in lists.iter().enumerate() {
                let hit = left.iter().any(|x| right.iter().any(|y| x.is_some() && x==y));
                let unknown = !left.is_empty() && !right.is_empty()
                    && (left.contains(&None) || right.contains(&None));
                if (!negative && hit) || (negative && !hit && !unknown) {
                    expected.push(vec![Value::Int32(a as i32),Value::Int32(b as i32)]);
                }
            }
        }
        let operator = if negative {"<>"} else {"="};
        let sql = format!("SELECT A.Unused,B.Unused FROM Items AS A,Items AS B WHERE A.Tags.Value{operator}B.Other.Value ORDER BY A.Unused,B.Unused");
        let result = run(&sql,&source).unwrap();
        prop_assert_eq!(result.rows(),expected);
    }
}

#[test]
fn multi_member_parameters_metadata_limits_and_every_execution_checkpoint_are_checked() {
    let source = Collections::new();
    let parsed = parse_query("PARAMETERS Delta LONG; SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT B.Id FROM Items AS B WHERE B.Other.Value=A.Tags.Value-A.Other.Value+Delta)",ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    for (delta, count) in [(0, 1), (100, 0)] {
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
                &[("Delta", Value::Int32(0))],
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
            &[("Delta", Value::Int32(0))],
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
        assert!(matches!(
            execute().err(),
            Some(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    source.changed.set(true);
    assert_eq!(
        plan.execute(
            &source,
            &[("Delta", Value::Int32(0))],
            ExecutionOptions::default()
        )
        .err(),
        Some(QueryError::SchemaChanged)
    );
}
