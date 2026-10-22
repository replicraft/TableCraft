use super::*;
use tablecraft_expressions::functions::conversions::text::NumberFormat;

#[test]
fn parent_membership_preserves_parent_rows_without_child_multiplication() {
    let source = Collections::new();
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Tags.Value=8 ORDER BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(
        run("SELECT Count(*) FROM Items WHERE Tags.Value=8", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int64(1)]]
    );
    assert_eq!(
        run(
            "SELECT Id,Count(*) FROM Items WHERE Tags.Value=7 GROUP BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1), Value::Int64(1)]]
    );
    assert_eq!(source.requests.borrow()[0], (vec![1], vec![0]));
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Tags.Value=8",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        2
    );
    let mut source = Collections::new();
    source.rows.push(source.rows[0].clone());
    source.lists.push(source.lists[0].clone());
    assert_eq!(
        run("SELECT Id FROM Items WHERE Tags.Value=8", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)], vec![Value::Int32(1)]]
    );
}
#[test]
fn independent_membership_atoms_and_negation_preserve_three_valued_logic() {
    let source = Collections::new();
    for sql in [
        "SELECT Id FROM Items WHERE Tags.Value=7 AND Tags.Value=8",
        "SELECT Id FROM Items WHERE Tags.Value IN (7,8) AND Other.Value=5",
        "SELECT Id FROM Items WHERE Tags.Value+Id=9",
    ] {
        assert_eq!(
            run(sql, &source).unwrap().rows(),
            [vec![Value::Int32(1)]],
            "{sql}"
        );
    }
    for sql in [
        "SELECT Id FROM Items WHERE NOT Tags.Value=7",
        "SELECT Id FROM Items WHERE Tags.Value<>7",
        "SELECT Id FROM Items WHERE Tags.Value NOT IN (7,8)",
        "SELECT Id FROM Items WHERE Tags.Value NOT BETWEEN 7 AND 8",
    ] {
        assert_eq!(
            run(sql, &source).unwrap().rows(),
            [vec![Value::Int32(2)]],
            "{sql}"
        );
    }
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Tags.Value=7 OR Id=3 ORDER BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1)], vec![Value::Int32(3)]]
    );
    assert_eq!(
        run("SELECT Id FROM Items WHERE Tags.Value IS NULL", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int32(3)]]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE NOT (Tags.Value=7 OR Other.Value=6)",
            &source
        )
        .unwrap()
        .rows(),
        [] as [Vec<Value>; 0]
    );
}
#[test]
fn parent_atoms_rewrite_correlated_scalar_slots_and_rebind_parameters() {
    let source = Collections::new();
    assert_eq!(run("SELECT A.Id FROM Items AS A WHERE EXISTS(SELECT 1 FROM Items AS B WHERE B.Tags.Value=A.Id+6) ORDER BY A.Id",&source).unwrap().rows(),[vec![Value::Int32(1)],vec![Value::Int32(2)]]);
    assert_eq!(
        run(
            "SELECT D.Id FROM (SELECT Id FROM Items WHERE Tags.Value=8) AS D",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1)]]
    );
    let parsed = parse_query(
        "PARAMETERS Needle LONG; SELECT Id FROM Items WHERE Tags.Value=Needle",
        ParseLimits::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &source).unwrap();
    for (needle, count) in [(8, 1), (9, 0)] {
        assert_eq!(
            prepared
                .execute(
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
}
#[test]
fn hidden_sort_member_binds_without_scanning_and_filters_the_same_child() {
    let source = Collections::new();
    let sql = "SELECT Id FROM Items WHERE Tags.Value=7 ORDER BY Tags.Value";
    let parsed = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.ordinary.get(), 0);
    assert!(source.requests.borrow().is_empty());
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int32(1)]]
    );
}
#[test]
fn matches_never_hide_later_member_or_logical_operand_errors() {
    let source = Collections::new();
    for sql in [
        "SELECT Id FROM Items WHERE 1 \\ (Tags.Value-8)>0",
        "SELECT Id FROM Items WHERE Id=1 OR 1 \\ (Tags.Value-8)>0",
        "SELECT Id FROM Items WHERE False AND 1 \\ (Tags.Value-8)>0",
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
    source.lists[0][0].push(Value::Text("bad".into()));
    source.ignore_errors = true;
    assert!(matches!(
        run("SELECT Id FROM Items WHERE Tags.Value=7", &source),
        Err(QueryError::Schema(_))
    ));
}
proptest! {
    #![proptest_config(proptest::test_runner::Config{failure_persistence:None,..Default::default()})]
    #[test]
    fn parent_memberships_match_independent_nullable_set_folds(values in prop::collection::vec(prop::collection::vec(prop::option::of(0_i32..4),0..8),0..10),a in 0_i32..4,b in 0_i32..4){
        let mut source=Collections::new();source.rows=values.iter().enumerate().map(|(i,_)|vec![Value::Int32(99),Value::Int32(i as i32)]).collect();source.lists=values.iter().map(|list|vec![list.iter().map(|value|value.map_or(Value::Null,Value::Int32)).collect(),vec![]]).collect();
        let expected=values.iter().enumerate().filter(|(_,list)|list.contains(&Some(a))&&list.contains(&Some(b))).map(|(i,_)|vec![Value::Int32(i as i32)]).collect::<Vec<_>>();
        let sql=format!("SELECT Id FROM Items WHERE Tags.Value={a} AND Tags.Value={b}");let actual=run(&sql,&source).unwrap();prop_assert_eq!(actual.rows(),expected);
        let expected=values.iter().enumerate().filter(|(_,list)|!list.contains(&Some(a))&&!list.contains(&None)).map(|(i,_)|vec![Value::Int32(i as i32)]).collect::<Vec<_>>();
        let sql=format!("SELECT Id FROM Items WHERE NOT Tags.Value={a}");let actual=run(&sql,&source).unwrap();prop_assert_eq!(actual.rows(),expected);
    }
}

#[test]
fn numeric_inspection_keeps_hidden_member_dependencies_and_parent_semantics() {
    let source = Collections::new();
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE IsNumeric(Tags.Value) ORDER BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(source.requests.borrow()[0], (vec![1], vec![0]));
    assert_eq!(
        run(
            "SELECT Count(*) FROM Items WHERE NOT IsNumeric(Tags.Value)",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(2)]]
    );
}

#[test]
fn date_conversion_retains_hidden_collection_dependencies() {
    let source = Collections::new();
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE DateValue(Tags.Value)=#1900-01-06# ORDER BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(source.requests.borrow()[0], (vec![1], vec![0]));
}

#[test]
fn weekday_retains_hidden_member_dependencies_in_parent_predicates() {
    let source = Collections::new();
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Weekday(Tags.Value)=7 ORDER BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(1)]]
    );
    assert_eq!(source.requests.borrow()[0], (vec![1], vec![0]));
}

#[test]
fn scalar_text_policy_changes_stop_expanded_parent_and_joined_parent_scans() {
    for sql in [
        "SELECT Id,Tags.Value FROM Items",
        "SELECT Id FROM Items WHERE Tags.Value=8",
        "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value=8 AND B.Other.Value=6",
    ] {
        let mut source = Collections::new();
        source.columns[0] = Column::new("Unused".into(), ScalarType::Text, true).unwrap();
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        source.policy_changed.set(true);
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .err(),
            Some(QueryError::SchemaChanged),
            "{sql}"
        );
        assert!(source.requests.borrow().is_empty(), "{sql}");
        assert_eq!(source.ordinary.get(), 0);
        source.policy_changed.set(false);
        assert!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .is_ok(),
            "{sql}"
        );
    }
}

#[test]
fn numeric_scalar_subtrees_keep_bits_in_collection_parent_criteria() {
    let source = Collections::new();
    for predicate in [
        "(1 And 2) And Tags.Value=8",
        "Tags.Value=8 And (1 And 2)",
        "Not (1 Or 2) And False And Tags.Value=8",
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate}");
        assert!(run(&sql, &source).unwrap().rows().is_empty(), "{sql}");
    }
    for predicate in [
        "(Id And 1) And Tags.Value=8",
        "((Id And 1) And Tags.Value=8) And Tags.Value=7",
        "Not (Id And 2) And Tags.Value=8",
        "Tags.Value And 1",
        "(Id And 1) And (Tags.Value=8 Or Other.Value=5)",
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate} ORDER BY Id");
        assert_eq!(
            run(&sql, &source).unwrap().rows(),
            [vec![Value::Int32(1)]],
            "{sql}"
        );
    }
    for predicate in [
        "0 And (1/0) And Tags.Value=8",
        "Tags.Value=8 Or (0 And (1/0))",
    ] {
        let sql = format!("SELECT Id FROM Items WHERE {predicate}");
        assert!(
            matches!(
                run(&sql, &source),
                Err(QueryError::Evaluation(
                    tablecraft_expressions::EvalError::DivisionByZero
                ))
            ),
            "{sql}"
        );
    }
}

#[test]
fn item_bitwise_operations_run_before_membership_folding() {
    let mut source = Collections::new();
    source.lists[0][0] = vec![Value::Int32(2)];
    source.lists[1][0] = vec![Value::Int32(1), Value::Int32(2)];
    source.lists[2][0] = vec![Value::Int32(0)];
    assert_eq!(
        run("SELECT Id FROM Items WHERE Tags.Value And 1", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int32(2)]]
    );
    assert_eq!(
        run("SELECT Id FROM Items WHERE Not Tags.Value", &source)
            .unwrap()
            .rows(),
        [
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE (Tags.Value And 1) And Other.Value=6",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int32(2)]]
    );
}

proptest! {
    #[test]
    fn generated_item_bits_and_parent_bits_match_independent_membership(
        lists in proptest::collection::vec(
            proptest::collection::vec(proptest::option::of(-8_i32..=8), 0..6),
            3,
        ),
    ) {
        let mut source = Collections::new();
        for (target, list) in source.lists.iter_mut().zip(&lists) {
            target[0] = list
                .iter()
                .map(|item| item.map_or(Value::Null, Value::Int32))
                .collect();
        }
        let selected = lists
            .iter()
            .enumerate()
            .filter_map(|(index, list)| {
                list.iter()
                    .any(|item| item.is_some_and(|item| item % 2 != 0))
                    .then_some(vec![Value::Int32(index as i32 + 1)])
            })
            .collect::<Vec<_>>();
        let actual = run(
            "SELECT Id FROM Items WHERE Tags.Value And 1 ORDER BY Id",
            &source,
        )
        .unwrap();
        prop_assert_eq!(actual.rows(), selected.as_slice());
        let selected = lists
            .iter()
            .enumerate()
            .filter_map(|(index, list)| {
                let parent_selected = index >= 1;
                let member_selected = list.iter()
                    .any(|item| item.is_some_and(|item| item > 0));
                (parent_selected && member_selected)
                    .then_some(vec![Value::Int32(index as i32 + 1)])
            })
            .collect::<Vec<_>>();
        let actual = run(
            "SELECT Id FROM Items WHERE (Id And 2) And Tags.Value>0 ORDER BY Id",
            &source,
        )
        .unwrap();
        prop_assert_eq!(actual.rows(), selected.as_slice());
    }
}

#[test]
fn text_member_operators_and_predicates_share_execution_policy_before_folding() {
    let mut source = Collections::new();
    source.values[0] = Column::new("Tags".into(), ScalarType::Text, true).unwrap();
    source.lists[0][0] = vec![Value::Text("2,5".into()), Value::Text("0".into())];
    source.lists[1][0] = vec![Value::Text("0".into())];
    source.lists[2][0] = vec![Value::Null];
    for sql in [
        "SELECT Id FROM Items WHERE Tags.Value And CInt(3) ORDER BY Id",
        "SELECT Id FROM Items WHERE Tags.Value ORDER BY Id",
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .is_err()
        );
        let result = plan
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    number_format: Some(NumberFormat::DeDe),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), [vec![Value::Int32(1)]], "{sql}");
    }
    let parsed = parse_query(
        "SELECT A.Id,B.Id FROM Items AS A,Items AS B WHERE A.Tags.Value And B.Tags.Value ORDER BY A.Id,B.Id",
        ParseLimits::default(),
    ).unwrap();
    let result = prepare(parsed.query(), &source)
        .unwrap()
        .execute(
            &source,
            &[],
            ExecutionOptions {
                number_format: Some(NumberFormat::DeDe),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.rows(), [vec![Value::Int32(1), Value::Int32(1)]]);
}
