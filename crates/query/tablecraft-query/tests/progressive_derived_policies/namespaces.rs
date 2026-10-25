use super::*;

#[test]
fn same_named_definitions_keep_each_leaf_namespace_and_restore_root_siblings() {
    let host = Host::new();
    let sql = "SELECT Id FROM Nested \
               UNION ALL SELECT Id FROM [one].Nested \
               UNION ALL SELECT Id FROM [two].Nested \
               UNION ALL SELECT Id FROM Same";
    let actual = stream(&host, sql, options(false)).unwrap();
    let expected =
        [10, 11, 12, 1, 2, 3, 100, 101, 102, 10, 11, 12].map(|value| vec![Value::Int32(value)]);
    assert_eq!(actual, expected);
    assert_eq!(host.root.scans.get(), 2);
    assert_eq!(host.one.scans.get(), 1);
    assert_eq!(host.two.scans.get(), 1);
}

#[test]
fn explicit_selector_in_external_definition_uses_operation_root_registry() {
    let mut host = Host::new();
    host.one.define("Remote", "SELECT Id FROM T IN 'two'");
    let sql = "SELECT Id FROM [one].Remote UNION ALL SELECT Id FROM Nested";
    let actual = stream(&host, sql, options(false)).unwrap();
    let expected = [100, 101, 102, 10, 11, 12].map(|value| vec![Value::Int32(value)]);
    assert_eq!(actual, expected);
    assert_eq!(host.one.scans.get(), 0);
    assert_eq!(host.two.scans.get(), 1);
    assert_eq!(host.root.scans.get(), 1);
}

#[test]
fn changed_external_key_or_provider_pointer_refuses_before_every_cursor() {
    for remap_pointer in [false, true] {
        let mut host = Host::new();
        // Identical metadata prevents a schema mismatch from masking pointer drift.
        host.two.policies = host.one.policies.clone();
        let syntax = parse_query("SELECT Id FROM [one].Nested", Default::default()).unwrap();
        let plan = prepare(syntax.query(), &host).unwrap();
        let lookups_before = host.binding_lookups.get();
        if remap_pointer {
            // Initial runtime registration still sees one. Its first schema
            // callback changes the registry before pre-cursor revalidation.
            host.one.remap_on_columns.set(true);
        } else {
            host.key.set(99);
        }
        let mut calls = 0;
        let error = plan
            .execute_progressive(&host, &[], options(false), &mut |_| {
                calls += 1;
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            StreamError::Query(QueryError::SchemaChanged)
        ));
        assert_eq!(calls, 0);
        assert_eq!(host.scans(), 0);
        assert_eq!(host.one.remaps.get(), usize::from(remap_pointer));
        let expected_lookups = if remap_pointer { 2 } else { 1 };
        assert_eq!(
            host.binding_lookups.get() - lookups_before,
            expected_lookups
        );
    }
}

#[test]
fn all_leaf_schemas_preflight_and_later_leaf_rechecks_after_earlier_eof() {
    let sql = "SELECT Id FROM Nested UNION ALL SELECT Id FROM [two].Nested";
    let host = Host::new();
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    host.two.schema_changed.set(true);
    let error = plan
        .execute_progressive(&host, &[], options(false), &mut |_| Ok::<_, ()>(()))
        .unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::SchemaChanged)
    ));
    assert_eq!(host.scans(), 0);

    let mut host = Host::new();
    host.root.mutate_after_scan = Some(host.two.schema_changed.clone());
    let plan = prepare(syntax.query(), &host).unwrap();
    let mut prefix = Vec::new();
    let error = plan
        .execute_progressive(&host, &[], options(false), &mut |row| {
            prefix.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap_err();
    assert!(matches!(
        error,
        StreamError::Query(QueryError::SchemaChanged)
    ));
    assert_eq!(
        prefix,
        [
            vec![Value::Int32(10)],
            vec![Value::Int32(11)],
            vec![Value::Int32(12)]
        ]
    );
    assert_eq!(host.root.scans.get(), 1);
    assert_eq!(host.two.scans.get(), 0);
}

#[test]
fn failed_or_capped_external_pipeline_does_not_leak_owner_to_a_later_execution() {
    let host = Host::new();
    let failed = "SELECT Id,1/0 FROM [one].Nested";
    assert!(matches!(
        stream(&host, failed, options(false)),
        Err(StreamError::Query(QueryError::Evaluation(
            EvalError::DivisionByZero
        )))
    ));
    let top = "SELECT TOP 1 Id FROM [one].Nested";
    assert_eq!(
        stream(&host, top, options(false)).unwrap(),
        [vec![Value::Int32(1)]]
    );
    let syntax = parse_query("SELECT Id FROM [two].Nested", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    let error = plan
        .execute_progressive(&host, &[], options(false), &mut |_| Err("sink failure"))
        .unwrap_err();
    assert!(matches!(error, StreamError::Sink("sink failure")));
    let sql = "SELECT Id FROM Same WHERE Id=10";
    assert_eq!(
        stream(&host, sql, options(false)).unwrap(),
        [vec![Value::Int32(10)]]
    );
    assert!(!host.active.get());
}
