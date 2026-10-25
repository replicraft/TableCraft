use super::*;
use tablecraft_access::reconstruct_saved_read;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{ExternalSourceRequest, MemorySource, MemoryTable, ResolvedExternalSource, SourceKey},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{
    query::{ReadBody, Statement},
    select::ExternalDatabase,
};

fn remote<'a>(path: Option<&'a str>, connection: Option<&'a str>) -> Fragment<'a> {
    Fragment {
        flag: Some(2),
        name1: path,
        expression: connection,
        ..row(4)
    }
}

fn source(name: &str) -> Fragment<'_> {
    Fragment {
        name1: Some(name),
        ..row(5)
    }
}

fn external(saved: &SavedSelect) -> &ExternalDatabase {
    let Statement::Read(read) = &saved.query().statement else {
        panic!("expected SELECT");
    };
    let ReadBody::Select(select) = &read.body else {
        panic!("expected SELECT body");
    };
    select.external.as_ref().unwrap()
}

#[test]
fn select_remote_metadata_preserves_exact_path_connection_and_original_fields() {
    for (path, connection) in [
        (Some("remote-token"), None),
        (Some("remote-token"), Some("MDB_FILE;")),
        (Some("remote-token"), Some("")),
        (None, Some("MDB_FILE;")),
        (Some(""), None),
        (Some("opaque ' é [x]; -- token"), Some("TYPE]; ' marker")),
    ] {
        let rows = [source("Items"), column("Id"), remote(path, connection)];
        let saved = compile(&rows).unwrap();
        let qualifier = external(&saved);
        assert_eq!(qualifier.database, path.unwrap_or(""));
        assert_eq!(qualifier.connection.as_deref(), connection);
        assert_eq!(qualifier.connection_bracketed, connection.is_some());
        let original = saved
            .definition()
            .rows()
            .iter()
            .find(|row| row.attribute() == Some(tablecraft_ace::QueryAttribute::RemoteDatabase))
            .unwrap();
        assert_eq!(original.name1(), path);
        assert_eq!(original.expression(), connection);
        assert_eq!(original.flag(), Some(2));
    }
}

#[test]
fn remote_join_and_parameter_reconstruction_matches_owned_external_syntax() {
    let rows = [
        Fragment {
            flag: Some(4),
            name1: Some("Floor"),
            ..row(2)
        },
        Fragment {
            name2: Some("a"),
            ..source("Items")
        },
        Fragment {
            name2: Some("b"),
            order: Some(&[0, 0, 0, 2]),
            ..source("Others")
        },
        Fragment {
            flag: Some(1),
            expression: Some("a.Id=b.Id"),
            name1: Some("a"),
            name2: Some("b"),
            ..row(7)
        },
        column("a.Id"),
        Fragment {
            expression: Some("a.Id>=Floor"),
            ..row(8)
        },
        remote(Some("remote-token"), None),
    ];
    let saved = compile(&rows).unwrap();
    let expected = parse_query("PARAMETERS Floor LONG; SELECT a.Id FROM (Items a INNER JOIN Others b ON a.Id=b.Id) IN 'remote-token' WHERE a.Id>=Floor", Default::default()).unwrap();
    assert_eq!(saved.query(), expected.query());
}

fn cross(pivot: &str) -> Vec<Fragment<'_>> {
    vec![
        Fragment {
            flag: Some(6),
            ..row(1)
        },
        source("Items"),
        Fragment {
            flag: Some(2),
            name1: Some("G"),
            ..column("0")
        },
        Fragment {
            flag: Some(0),
            order: Some(&[0, 0, 0, 2]),
            ..column("Count(*)")
        },
        Fragment {
            flag: Some(1),
            order: Some(&[0, 0, 0, 3]),
            ..column(pivot)
        },
        Fragment {
            flag: Some(2),
            expression: Some("0"),
            ..row(9)
        },
        remote(Some("remote-token"), None),
    ]
}

fn read(rows: &[Fragment<'_>], family: u32) -> Result<SavedSelect, E> {
    reconstruct_saved_read(
        definition(rows, family),
        Default::default(),
        &mut budget(),
        &|| false,
    )
}

#[test]
fn fixed_and_dynamic_crosstabs_retain_global_external_metadata() {
    for pivot in ["Id", "Id IN (1,2)"] {
        let saved = read(&cross(pivot), 0x10).unwrap();
        let sql = format!(
            "TRANSFORM Count(*) SELECT 0 AS G FROM Items IN 'remote-token' GROUP BY 0 PIVOT {pivot}"
        );
        assert_eq!(
            saved.query(),
            parse_query(&sql, Default::default()).unwrap().query()
        );
        let host = ExternalHost::new();
        let plan = prepare(saved.query(), &host).unwrap();
        assert_eq!(host.scans.get(), 0);
        let result = plan
            .execute(&host, &[], ExecutionOptions::default())
            .unwrap();
        assert_eq!(result.columns(), ["G", "1", "2"]);
        assert_eq!(
            result.rows(),
            &[vec![Value::Int64(0), Value::Int64(1), Value::Int64(1)]]
        );
    }
}

#[test]
fn duplicate_unknown_payload_and_sourcefree_remote_metadata_remain_rejected() {
    for flag in [None, Some(0), Some(1), Some(3), Some(-1)] {
        let invalid = Fragment {
            flag,
            ..remote(Some("remote-token"), None)
        };
        assert_eq!(
            compile(&[source("Items"), column("Id"), invalid]).err(),
            Some(E::Unsupported)
        );
    }
    for invalid in [
        Fragment {
            extra: Some(0),
            ..remote(Some("remote-token"), None)
        },
        Fragment {
            name2: Some("alias"),
            ..remote(Some("remote-token"), None)
        },
    ] {
        assert_eq!(
            compile(&[source("Items"), column("Id"), invalid]).err(),
            Some(E::Unsupported)
        );
    }
    assert_eq!(
        compile(&[source("Items"), column("Id"), remote(None, None)]).err(),
        Some(E::InvalidMetadata)
    );
    assert_eq!(
        compile(&[column("1"), remote(Some("remote-token"), None)]).err(),
        Some(E::InvalidMetadata)
    );
    assert_eq!(
        compile(&[
            source("Items"),
            column("Id"),
            remote(Some("remote-token"), None),
            remote(Some("remote-token"), None)
        ])
        .err(),
        Some(E::InvalidMetadata)
    );

    let per_table = Fragment {
        expression: Some("remote-token"),
        ..source("Items")
    };
    assert_eq!(
        compile(&[per_table, column("Id")]).err(),
        Some(E::Unsupported)
    );
}

#[test]
fn remote_strings_obey_shared_syntax_budget_and_cancellation_limits() {
    let token = "x".repeat(128);
    let rows = [source("Items"), column("Id"), remote(Some(&token), None)];
    let result = reconstruct_saved_select(
        definition(&rows, 0),
        ParseLimits {
            max_bytes: 64,
            ..Default::default()
        },
        &mut budget(),
        &|| false,
    );
    assert_eq!(result.err(), Some(E::SyntaxLimit));

    let cancelled = reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut budget(),
        &|| true,
    );
    assert_eq!(cancelled.err(), Some(E::Cancelled));
    let exhausted = reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut ResourceBudget::new(0, 0, 0),
        &|| false,
    );
    assert!(matches!(exhausted, Err(E::Limit(_))));
}

struct ExternalHost {
    data: MemorySource,
    scans: Cell<usize>,
    enabled: Cell<bool>,
}

impl ExternalHost {
    fn new() -> Self {
        let table = MemoryTable::new(
            "Items".into(),
            vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
        )
        .unwrap();
        Self {
            data: MemorySource::new(vec![table]).unwrap(),
            scans: Cell::new(0),
            enabled: Cell::new(true),
        }
    }
}

impl Source for ExternalHost {
    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        Ok((self.enabled.get()
            && request.database() == "remote-token"
            && request.connection().is_none())
        .then(|| ResolvedExternalSource::new(SourceKey::new(17), self)))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.data.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        self.data.scan(table, visitor)
    }
}

#[test]
fn reconstructed_external_queries_require_explicit_mapping_and_revalidate_before_rows() {
    let saved = compile(&[
        source("Items"),
        column("Id"),
        remote(Some("remote-token"), None),
    ])
    .unwrap();
    let host = ExternalHost::new();
    let plan = prepare(saved.query(), &host).unwrap();
    assert_eq!(host.scans.get(), 0);
    assert_eq!(
        plan.execute(&host, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        &[vec![Value::Int32(1)], vec![Value::Int32(2)]]
    );

    let with_empty_connection = compile(&[
        source("Items"),
        column("Id"),
        remote(Some("remote-token"), Some("")),
    ])
    .unwrap();
    assert!(prepare(with_empty_connection.query(), &host).is_err());

    host.enabled.set(false);
    let scans = host.scans.get();
    assert!(matches!(
        plan.execute(&host, &[], ExecutionOptions::default()),
        Err(QueryError::SchemaChanged)
    ));
    assert_eq!(host.scans.get(), scans);
    assert!(prepare(saved.query(), &host).is_err());
}

#[test]
fn labelled_union_arms_already_preserve_external_scopes_without_global_remote_rows() {
    let rows = [
        Fragment {
            flag: Some(9),
            ..row(1)
        },
        Fragment {
            name2: Some("X7YZ_____1"),
            ..Fragment {
                expression: Some("SELECT Id FROM Items IN 'remote-token'"),
                ..row(5)
            }
        },
        Fragment {
            name2: Some("X7YZ_____2"),
            order: Some(&[0, 0, 0, 2]),
            expression: Some("SELECT 3 AS Id"),
            ..row(5)
        },
    ];
    let saved = read(&rows, 0x80).unwrap();
    let expected = parse_query(
        "SELECT Id FROM Items IN 'remote-token' UNION ALL SELECT 3 AS Id",
        Default::default(),
    )
    .unwrap();
    assert_eq!(saved.query(), expected.query());
    let host = ExternalHost::new();
    let result = prepare(saved.query(), &host)
        .unwrap()
        .execute(&host, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(result.rows().len(), 3);

    let mut with_global = Vec::from(rows);
    with_global.push(remote(Some("remote-token"), None));
    assert_eq!(read(&with_global, 0x80).err(), Some(E::Unsupported));
}

#[test]
fn completed_remote_reconstruction_still_checks_final_cancellation_and_exact_quotas() {
    let rows = [
        source("Items"),
        column("Id"),
        remote(Some("remote-token"), Some("")),
    ];
    let checks = Cell::new(0);
    reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    let complete_checks = checks.get();
    checks.set(0);
    let cancellation = || {
        checks.set(checks.get() + 1);
        checks.get() == complete_checks
    };
    assert_eq!(
        reconstruct_saved_select(
            definition(&rows, 0),
            Default::default(),
            &mut budget(),
            &cancellation
        )
        .err(),
        Some(E::Cancelled)
    );

    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut full = budget();
        let before = full.remaining(resource);
        reconstruct_saved_select(definition(&rows, 0), Default::default(), &mut full, &|| {
            false
        })
        .unwrap();
        let consumed = before - full.remaining(resource);
        assert!(consumed > 0);

        let mut limited = ResourceBudget::new(
            if resource == Resource::Bytes {
                consumed - 1
            } else {
                64 * 1024 * 1024
            },
            if resource == Resource::Rows {
                consumed - 1
            } else {
                16_384
            },
            if resource == Resource::Steps {
                consumed - 1
            } else {
                1_000_000
            },
        );
        let result = reconstruct_saved_select(
            definition(&rows, 0),
            Default::default(),
            &mut limited,
            &|| false,
        );
        assert!(matches!(result, Err(E::Limit(error)) if error.resource == resource));
    }
}
