use super::*;

#[test]
fn final_key_policy_and_selected_extrema_policy_are_different_populations() {
    let source = Data::new();
    let resolver = Resolver::new(&source);
    let sql =
        format!("SELECT {KEY} AS K,Count(*),Min(C),Max(C),Len(K),VarType(K) FROM T GROUP BY {KEY}");
    let expected = vec![vec![
        Value::Text("10".into()),
        Value::Int64(3),
        Value::Text("yz".into()),
        Value::Text("ab".into()),
        Value::Int32(2),
        Value::Int16(8),
    ]];
    for declared in [false, true] {
        resolver.trace.borrow_mut().clear();
        assert_eq!(
            stream(&source, &sql, options(&resolver, declared)).unwrap(),
            expected
        );
        let trace = resolver.trace.borrow();
        let key_compares: Vec<_> = trace
            .iter()
            .enumerate()
            .filter_map(|(index, event)| matches!(event, Event::Compare(2)).then_some(index))
            .collect();
        let operand_compares: Vec<_> = trace
            .iter()
            .enumerate()
            .filter_map(|(index, event)| matches!(event, Event::Compare(3)).then_some(index))
            .collect();
        assert!(!key_compares.is_empty());
        assert!(!operand_compares.is_empty());
        assert!(key_compares.iter().max().unwrap() < operand_compares.iter().min().unwrap());
        assert!(
            trace
                .iter()
                .all(|event| matches!(event, Event::Resolve(2 | 3) | Event::Compare(2 | 3)))
        );
    }
}

#[test]
fn having_only_string_extrema_does_not_supply_key_or_selected_extrema_policy() {
    let source = Data::new();
    let resolver = Resolver::new(&source);
    let sql = format!("SELECT {KEY} AS K,Max(C) FROM T GROUP BY {KEY} HAVING Min(C)='yz'");
    let expected = vec![vec![Value::Text("10".into()), Value::Text("ab".into())]];
    assert_eq!(
        stream(&source, &sql, options(&resolver, false)).unwrap(),
        expected
    );
    let trace = resolver.trace.borrow();
    assert!(trace.iter().any(|event| matches!(event, Event::Compare(2))));
    assert!(trace.iter().any(|event| matches!(event, Event::Compare(3))));
    assert!(
        trace
            .iter()
            .all(|event| matches!(event, Event::Resolve(2 | 3) | Event::Compare(2 | 3)))
    );
}

#[test]
fn rejected_having_partitions_keys_without_selected_provider_or_semantic_work() {
    let mut source = Data::new();
    source.policies[4] = Some(TextCollationId::from_host(999));
    let resolver = Resolver::new(&source);
    for predicate in ["False", "NULL"] {
        resolver.trace.borrow_mut().clear();
        let sql =
            format!("SELECT ChrW(55296),Min(C),Sum(1/0) FROM T GROUP BY {KEY} HAVING {predicate}");
        let actual = stream(&source, &sql, options(&resolver, false)).unwrap();
        assert!(actual.is_empty());
        let trace = resolver.trace.borrow();
        assert!(trace.iter().any(|event| matches!(event, Event::Compare(2))));
        assert!(
            trace
                .iter()
                .all(|event| matches!(event, Event::Resolve(2) | Event::Compare(2)))
        );
    }
    let sql = format!("SELECT Count(*) FROM T GROUP BY {KEY} HAVING False AND Min(C)='x'");
    let error = stream(&source, &sql, options(&resolver, false)).unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::CollationRequired))
    ));
    assert!(
        resolver
            .trace
            .borrow()
            .iter()
            .any(|event| matches!(event, Event::Resolve(999)))
    );
}

#[test]
fn final_partition_replays_checked_prefixes_instead_of_merging_provisional_totals() {
    let mut source = Data::new();
    source.rows[0][1] = Value::Int64(i64::MAX);
    source.rows[1][1] = Value::Int64(1);
    source.rows[2][1] = Value::Int64(-1);
    let resolver = Resolver::new(&source);
    let sql = format!("SELECT {KEY},Sum(N) FROM T GROUP BY {KEY}");
    let error = stream(&source, &sql, options(&resolver, false)).unwrap_err();
    // The exact final total fits, but the existing integral prefix must overflow.
    assert!(matches!(
        error,
        StreamError::Query(QueryError::Evaluation(EvalError::Overflow))
    ));
    assert!(source.eof.get());
    let sql = format!("SELECT Sum(N) FROM T GROUP BY {KEY} HAVING False");
    assert!(
        stream(&source, &sql, options(&resolver, false))
            .unwrap()
            .is_empty()
    );
    let sql = format!("SELECT Count(*) FROM T GROUP BY {KEY} HAVING Sum(N)>0");
    assert!(matches!(
        stream(&source, &sql, options(&resolver, false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::Overflow
        )))
    ));
}
