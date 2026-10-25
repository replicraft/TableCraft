//! Explicit per-table selectors reuse authorized namespaces without changing owners.
#[allow(dead_code)]
#[path = "external_named_sources/support.rs"]
mod named;
#[allow(dead_code)]
#[path = "external_sources/support.rs"]
mod providers;

use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{
    ExecutionOptions, QueryError, execute_with_schema_discovery, prepare,
    source::{TextCollationId, TextPolicy},
};
use tablecraft_sql::{
    parse_query,
    query::{ReadBody, Statement},
    select::TableSource,
};

#[test]
fn mixed_root_and_two_external_table_leaves_keep_aliases_and_row_namespaces() {
    let host = providers::Host::new(false);
    let sql = concat!(
        "SELECT R.Id,A.Id,B.Id FROM (T R INNER JOIN [remote].T A ON R.Id=A.Id*10) ",
        "INNER JOIN [second].T B ON B.Id=A.Id*100 WHERE A.Id>=2",
    );
    assert_eq!(
        providers::run(&host, sql).unwrap(),
        [vec![Value::Int32(20), Value::Int32(2), Value::Int32(200)]],
    );
    assert_eq!(host.remote.projections.get(), 1);
    assert_eq!(host.second.projections.get(), 1);

    let host = providers::Host::new(false);
    assert_eq!(
        providers::run(
            &host,
            "SELECT R.*,A.* FROM T R,[remote].T A WHERE R.Id=A.Id*10 ORDER BY R.Id"
        )
        .unwrap(),
        [
            vec![
                Value::Int32(10),
                Value::Text("value10".into()),
                Value::Int32(1),
                Value::Text("value1".into())
            ],
            vec![
                Value::Int32(20),
                Value::Text("value20".into()),
                Value::Int32(2),
                Value::Text("value2".into())
            ],
        ],
    );
}

#[test]
fn explicit_leaf_overrides_global_in_but_caller_subqueries_keep_root_owner() {
    let host = providers::Host::new(false);
    assert_eq!(
        providers::run(
            &host,
            concat!(
                "SELECT D.Id,A.Id,(SELECT Max(Id) FROM T) AS RootMax ",
                "FROM T D,[remote].T A IN 'second' WHERE D.Id=A.Id*100 ORDER BY A.Id",
            )
        )
        .unwrap(),
        [
            vec![Value::Int32(100), Value::Int32(1), Value::Int32(20)],
            vec![Value::Int32(200), Value::Int32(2), Value::Int32(20)],
        ],
    );
}

#[test]
fn leaf_selection_is_exact_metadata_only_and_revisions_are_pinned_before_scans() {
    for token in ["REMOTE", "remote ", "missing"] {
        let host = providers::Host::new(false);
        let syntax =
            parse_query(&format!("SELECT Id FROM [{token}].T"), Default::default()).unwrap();
        assert!(matches!(
            prepare(syntax.query(), &host),
            Err(QueryError::Unsupported)
        ));
        assert_eq!(host.local.scans.get(), 0);
        assert_eq!(host.remote.projections.get(), 0);
    }
    for changed_schema in [false, true] {
        let host = providers::Host::new(false);
        let syntax =
            parse_query("SELECT R.Id,A.Id FROM T R,[remote].T A", Default::default()).unwrap();
        let plan = prepare(syntax.query(), &host).unwrap();
        assert_eq!(host.local.scans.get(), 0);
        assert_eq!(host.remote.projections.get(), 0);
        if changed_schema {
            host.remote.changed_schema.set(true);
            // Schema is revalidated per leaf when reached. Earlier unchanged
            // root reads are allowed; the changed target must not be scanned.
            host.allow_rows();
        } else {
            host.key.set(19);
        }
        assert!(matches!(
            plan.execute(&host, &[], ExecutionOptions::default()),
            Err(QueryError::SchemaChanged)
        ));
        if !changed_schema {
            assert_eq!(host.local.scans.get(), 0);
        }
        assert_eq!(host.remote.projections.get(), 0);
        assert_eq!(host.remote.scans.get(), 0);
        assert_eq!(host.remote.collection_scans.get(), 0);
    }
}

#[test]
fn repeated_leaves_keep_projection_constraints_caches_and_text_policies_isolated() {
    let host = providers::Host::new(false);
    let syntax = parse_query(
        "SELECT Label FROM [remote].T WHERE Id=2",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    assert_eq!(
        plan.output_text_policies(),
        Some([TextPolicy::Column(TextCollationId::from_host(2))].as_slice())
    );
    assert_eq!(host.remote.projections.get(), 0);
    host.allow_rows();
    assert_eq!(
        plan.execute(&host, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Text("value2".into())]]
    );
    assert!(host.remote.saw_constraint.get());
    assert_eq!(host.local.scans.get(), 0);
    assert_eq!(host.second.projections.get(), 0);

    let host = providers::Host::new(false);
    assert_eq!(
        providers::run(
            &host,
            concat!(
                "SELECT A.Id,B.Id,C.Id FROM [remote].T A,[remote].T B,[second].T C ",
                "WHERE A.Id=2 AND B.Id=2 AND C.Id=200",
            )
        )
        .unwrap(),
        [vec![Value::Int32(2), Value::Int32(2), Value::Int32(200)]]
    );
    assert_eq!(host.local.scans.get(), 0);
}

#[test]
fn target_named_definitions_keep_lexical_subquery_and_domain_owners() {
    let mut host = named::Host::new();
    for node in [&mut host.one, &mut host.two] {
        node.define(
            "Owned",
            "SELECT Id,(SELECT Max(Id) FROM T) AS Local,DMax('Id','T') AS D FROM Same",
        );
    }
    assert_eq!(
        named::run(
            &host,
            concat!(
                "SELECT A.Id,B.Id,A.Local,B.Local,A.D,B.D,DMax('Id','T') AS Root ",
                "FROM [one].Owned A,[two].Owned B",
            )
        )
        .unwrap()
        .rows(),
        [vec![
            Value::Int32(1),
            Value::Int32(2),
            Value::Int32(1),
            Value::Int32(2),
            Value::Int32(1),
            Value::Int32(2),
            Value::Int32(10),
        ]]
    );

    let mut host = named::Host::new();
    host.one.define(
        "Cross",
        "SELECT Id,(SELECT Max(Id) FROM T) AS Owner FROM [two].Same",
    );
    assert_eq!(
        named::run(&host, "SELECT * FROM [one].[Cross]")
            .unwrap()
            .rows(),
        [vec![Value::Int32(2), Value::Int32(1)]]
    );
}

#[test]
fn target_named_parameters_and_dynamic_schema_discovery_use_leaf_namespaces() {
    let mut host = named::Host::new();
    host.one
        .define("P", "PARAMETERS Floor LONG; SELECT Id+Floor AS V FROM T");
    let syntax = parse_query("SELECT V FROM [one].P", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    assert_eq!(host.scans(), 0);
    assert_eq!(
        plan.execute(&host, &[("Floor", Value::Int32(3))], named::options())
            .unwrap()
            .rows(),
        [vec![Value::Int32(4)]]
    );

    let host = named::Host::new();
    let syntax = parse_query(
        "SELECT A.[1],B.[2] FROM [one].D A,[two].D B",
        Default::default(),
    )
    .unwrap();
    assert!(prepare(syntax.query(), &host).is_err());
    assert_eq!(host.scans(), 0);
    let result =
        execute_with_schema_discovery(syntax.query(), &host, &[], named::options()).unwrap();
    assert_eq!(
        result.rows(),
        [vec![
            ExpressionValue::Value(Value::Int64(1)),
            ExpressionValue::Value(Value::Int64(1)),
        ]]
    );
    assert_eq!(host.root.scans.get(), 0);
    assert_eq!(host.one.scans.get(), 1);
    assert_eq!(host.two.scans.get(), 1);
}

#[test]
fn leaf_failures_remain_buffered_and_explicit_retry_reloads_the_target() {
    let host = providers::Host::new(false);
    let syntax = parse_query("SELECT Id FROM [remote].T", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    host.allow_rows();
    host.remote.fail_late.set(true);
    assert!(matches!(
        plan.execute(&host, &[], ExecutionOptions::default()),
        Err(QueryError::Source)
    ));
    host.remote.fail_late.set(false);
    assert_eq!(
        plan.execute(&host, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );
}

#[test]
fn caller_constructed_leaf_selectors_cannot_bypass_owning_token_validation() {
    for selector in ["", "remote\n", "remote\0", "remote\u{7f}"] {
        let host = providers::Host::new(false);
        let mut query = parse_query("SELECT Id FROM T", Default::default())
            .unwrap()
            .into_query();
        let Statement::Read(read) = &mut query.statement else {
            unreachable!()
        };
        let ReadBody::Select(select) = &mut read.body else {
            unreachable!()
        };
        let TableSource::Table(table) = &mut select.from[0] else {
            unreachable!()
        };
        table.database = Some(selector.into());
        assert!(matches!(
            prepare(&query, &host),
            Err(QueryError::Unsupported)
        ));
        assert_eq!(host.lookups.get(), 0);
        assert_eq!(host.local.scans.get(), 0);
        assert_eq!(host.remote.projections.get(), 0);
    }
}
