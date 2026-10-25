use super::*;

#[test]
fn interleaved_groups_finish_independent_late_string_policy_cohorts_after_eof() {
    let source = Data::new(&[
        (2, 1, "15"),
        (1, 4, "45"),
        (2, 2, "15"),
        (1, 5, "45"),
        (2, 3, "15"),
        (1, 6, "45"),
    ]);
    let expression = "IIf(Id Mod 3=0,B,Id)&IIf(True,0,A)";
    let sql = format!("SELECT G,Min({expression}),Max({expression}) FROM T GROUP BY G");
    let expected = vec![
        vec![
            Value::Int32(2),
            Value::Text("20".into()),
            Value::Text("10".into()),
        ],
        vec![
            Value::Int32(1),
            Value::Text("50".into()),
            Value::Text("40".into()),
        ],
    ];
    for declared in [false, true] {
        let resolver = Resolver {
            source: &source,
            hits: Cell::new(0),
        };
        let actual = stream(
            &source,
            &sql,
            ExecutionOptions {
                collation: Some(&ORDINAL),
                collation_resolver: Some(&resolver),
                ..options(declared)
            },
        )
        .unwrap();
        assert_eq!(actual, expected);
        assert!(resolver.hits.get() > 0);

        let syntax = parse_query(&sql, Default::default()).unwrap();
        let buffered = prepare(syntax.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&ORDINAL),
                    collation_resolver: Some(&resolver),
                    ..options(declared)
                },
            )
            .unwrap();
        assert_eq!(buffered.rows(), actual);
    }
}

#[test]
fn checked_eof_failure_prevents_provider_activation_and_every_group_output() {
    let mut source = Data::new(&[(2, 1, "15"), (2, 2, "15"), (2, 3, "15")]);
    source.fail_eof = true;
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let expression = "IIf(Id=3,B,Id)&IIf(True,0,A)";
    let sql = format!("SELECT G,Min({expression}) FROM T GROUP BY G");
    let error = stream(
        &source,
        &sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap_err();
    assert!(matches!(error, StreamError::Query(QueryError::Source)));
    assert_eq!(resolver.hits.get(), 0);
}

#[test]
fn singleton_groups_do_not_activate_unused_required_text_comparers() {
    let source = Data::new(&[(2, 1, "first"), (1, 2, "second")]);
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let sql = "SELECT G,Len(Min(B)),Len(Max(B)) FROM T GROUP BY G";
    let actual = stream(
        &source,
        sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap();
    let expected = vec![
        vec![Value::Int32(2), Value::Int32(5), Value::Int32(5)],
        vec![Value::Int32(1), Value::Int32(6), Value::Int32(6)],
    ];
    assert_eq!(actual, expected);
    assert_eq!(resolver.hits.get(), 0);
}

#[test]
fn actual_string_policy_conflicts_remain_refused_after_eof() {
    let source = Data::new(&[(2, 1, "b"), (2, 2, "a")]);
    let resolver = Resolver {
        source: &source,
        hits: Cell::new(0),
    };
    let sql = "SELECT G,Min(IIf(Id=1,A,B)) FROM T GROUP BY G";
    let error = stream(
        &source,
        sql,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..options(false)
        },
    )
    .unwrap_err();
    assert!(source.eof.get());
    assert!(matches!(
        error,
        StreamError::Query(QueryError::CollationConflict)
    ));
    assert_eq!(resolver.hits.get(), 0);
}
