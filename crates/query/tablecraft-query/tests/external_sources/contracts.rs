use super::support::{Host, run};
use tablecraft_query::source::ExternalSourceRequest;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, execute_with_schema_discovery, prepare,
};
use tablecraft_sql::parse_query;

#[test]
fn selectors_are_exact_and_do_not_treat_tokens_as_open_requests() {
    let host = Host::new(false);
    for token in ["REMOTE", "remote ", "missing"] {
        let sql = format!("SELECT Id FROM T IN '{token}'");
        let syntax = parse_query(&sql, Default::default()).unwrap();
        assert!(prepare(syntax.query(), &host).is_err());
    }

    assert!(
        host.external_source(ExternalSourceRequest::new("remote", Some(""), false))
            .unwrap()
            .is_none()
    );
    assert!(
        host.external_source(ExternalSourceRequest::new("remote", None, true))
            .unwrap()
            .is_none()
    );
    assert_eq!(host.remote.projections.get(), 0);
}

#[test]
fn missing_or_remapped_bindings_fail_before_any_root_or_target_records() {
    for remap in [false, true] {
        let host = Host::new(false);
        let syntax = parse_query(
            "SELECT R.Id FROM T R,(SELECT Id FROM T IN 'remote') E",
            Default::default(),
        )
        .unwrap();
        let plan = prepare(syntax.query(), &host).unwrap();
        if remap {
            host.key.set(8);
        } else {
            host.enabled.set(false);
        }

        assert!(matches!(
            plan.execute(&host, &[], ExecutionOptions::default()),
            Err(QueryError::SchemaChanged)
        ));
        assert_eq!(host.local.scans.get(), 0);
        assert_eq!(host.remote.projections.get(), 0);
    }
}

#[test]
fn changed_target_schema_and_late_failure_never_publish_a_complete_result() {
    let host = Host::new(false);
    let syntax = parse_query("SELECT Id FROM T IN 'remote'", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    host.remote.changed_schema.set(true);
    assert!(matches!(
        plan.execute(&host, &[], ExecutionOptions::default()),
        Err(QueryError::SchemaChanged)
    ));
    assert_eq!(host.remote.projections.get(), 0);

    host.remote.changed_schema.set(false);
    host.remote.fail_late.set(true);
    host.allow_rows();
    assert!(matches!(
        plan.execute(&host, &[], ExecutionOptions::default()),
        Err(QueryError::Source)
    ));
    host.remote.fail_late.set(false);
    assert_eq!(run(&host, "SELECT Id FROM T IN 'remote'").unwrap().len(), 2);
}

#[test]
fn discovery_cannot_reuse_completed_cells_after_external_revision_changes() {
    let mut host = Host::new(false);
    host.definition(
        "Pivoted",
        "TRANSFORM Count(*) SELECT 1 AS G FROM T IN 'remote' GROUP BY 1 PIVOT Id",
    );
    let syntax = parse_query(
        "SELECT [1] FROM Pivoted UNION ALL SELECT [1] FROM Pivoted",
        Default::default(),
    )
    .unwrap();
    host.remote.change_key_after_scan.set(true);
    host.allow_rows();

    assert!(matches!(
        execute_with_schema_discovery(syntax.query(), &host, &[], ExecutionOptions::default()),
        Err(QueryError::SchemaChanged)
    ));
    assert_eq!(host.remote.projections.get(), 1);
}

#[test]
fn cancellation_and_exhausted_query_quotas_precede_external_revalidation() {
    let host = Host::new(false);
    let syntax = parse_query("SELECT Id FROM T IN 'remote'", Default::default()).unwrap();
    let plan = prepare(syntax.query(), &host).unwrap();
    let lookups = host.lookups.get();
    let cancelled = || true;
    let options = ExecutionOptions {
        cancelled: Some(&cancelled),
        ..Default::default()
    };
    assert!(matches!(
        plan.execute(&host, &[], options),
        Err(QueryError::Cancelled)
    ));
    assert_eq!(host.lookups.get(), lookups);

    for limits in [
        tablecraft_query::QueryLimits {
            steps: 0,
            ..Default::default()
        },
        tablecraft_query::QueryLimits {
            bytes: 0,
            ..Default::default()
        },
    ] {
        let options = ExecutionOptions {
            limits,
            ..Default::default()
        };
        assert!(matches!(
            plan.execute(&host, &[], options),
            Err(QueryError::Limit(_))
        ));
    }
    assert_eq!(host.remote.projections.get(), 0);
}

#[test]
fn caller_constructed_external_select_without_from_is_refused_before_resolution() {
    use tablecraft_sql::query::{ReadBody, Statement};

    let host = Host::new(false);
    let syntax = parse_query("SELECT Id FROM T IN 'remote'", Default::default()).unwrap();
    let mut query = syntax.query().clone();
    let Statement::Read(read) = &mut query.statement else {
        panic!("expected read query");
    };
    let ReadBody::Select(select) = &mut read.body else {
        panic!("expected SELECT");
    };
    select.from.clear();

    assert!(matches!(
        prepare(&query, &host),
        Err(QueryError::Unsupported)
    ));
    assert_eq!(host.lookups.get(), 0);
}
