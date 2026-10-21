use super::*;

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
fn unsupported_parent_shapes_fail_without_scanning() {
    let source = Collections::new();
    for sql in [
        "SELECT Id FROM Items WHERE Tags.Value=Other.Value",
        "SELECT Id FROM Items WHERE Tags.Value=7 ORDER BY Tags.Value",
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
