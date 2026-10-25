use super::*;

#[test]
fn intermediate_extrema_keep_unknown_declaration_despite_a_known_numeric_descriptor() {
    let host = Host::new();
    for aggregate in ["Min", "Max"] {
        for prepared_declared in [false, true] {
            let sql = format!("SELECT M+'2' FROM (SELECT {aggregate}(Id) AS M FROM T) Q");
            let syntax = parse_query(&sql, Default::default()).unwrap();
            let plan = prepare_with_options(
                syntax.query(),
                &host,
                PreparationOptions {
                    declared_arithmetic: prepared_declared,
                    ..Default::default()
                },
            )
            .unwrap();
            let error = plan
                .execute_progressive(&host, &[], options(true), &mut |_| Ok::<_, ()>(()))
                .unwrap_err();
            assert!(
                matches!(error, StreamError::Query(QueryError::ArithmeticDeclaration)),
                "{aggregate}: {error:?}"
            );
            host.assert_no_snapshot();
        }
        let sql = format!("SELECT CLng(M)+CLng(2) FROM (SELECT {aggregate}(Id) AS M FROM T) Q");
        let expected = if aggregate == "Min" { 12 } else { 13 };
        assert_eq!(
            stream(&host, &sql, options(true)).unwrap(),
            [vec![Value::Int32(expected)]]
        );
    }
}

#[test]
fn inner_complete_operand_policies_form_a_new_outer_population_without_static_phantoms() {
    let host = Host::new();
    let resolver = Resolver {
        host: &host,
        hits: Cell::new(0),
    };
    let sql = "SELECT Min(S),Max(S),Count(*) FROM \
               (SELECT G,Min(V) AS S FROM \
               (SELECT G,IIf(Id=11,B,Id)&IIf(True,0,A) AS V FROM T) X GROUP BY G) Q";
    let actual = stream(
        &host,
        sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap();
    // Singleton inner groups emit100(Default-complete) and150(B-complete).
    // Their new outer population selects B's all-equal provider and first100.
    assert_eq!(
        actual,
        [vec![
            Value::Text("100".into()),
            Value::Text("100".into()),
            Value::Int64(2)
        ]]
    );
    assert!(resolver.hits.get() > 0);
}

#[test]
fn text_group_key_and_operand_provenance_do_not_exchange_across_a_barrier() {
    let host = Host::new();
    let resolver = Resolver {
        host: &host,
        hits: Cell::new(0),
    };
    let key = "IIf(Id=11,B,Id)&IIf(True,0,A)";
    let sql = format!(
        "SELECT Min(S),Min(S)='yz' FROM (SELECT {key} AS K,Min(C) AS S FROM T GROUP BY {key}) Q"
    );
    let actual = stream(
        &host,
        &sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap();
    assert_eq!(
        actual,
        [vec![Value::Text("yz".into()), Value::Boolean(true)]]
    );
    assert!(resolver.hits.get() > 0);
}

#[test]
fn same_named_stateful_owners_and_explicit_registry_leave_root_siblings_isolated() {
    let mut host = Host::new();
    host.one
        .define("Remote", "SELECT Min(Id) AS M FROM T IN 'two'");
    let sql = "SELECT M FROM Same UNION ALL SELECT M FROM [one].Nested \
               UNION ALL SELECT M FROM [two].Same UNION ALL SELECT M FROM Same \
               UNION ALL SELECT M FROM [one].Remote UNION ALL SELECT M FROM Same";
    let actual = stream(&host, sql, options(false)).unwrap();
    let expected = [10, 1, 100, 10, 100, 10].map(|value| vec![Value::Int32(value)]);
    assert_eq!(actual, expected);
    assert_eq!(host.root.scans.get(), 3);
    assert_eq!(host.one.scans.get(), 1);
    assert_eq!(host.two.scans.get(), 2);
}
