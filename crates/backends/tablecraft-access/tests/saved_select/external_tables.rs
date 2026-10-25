//! Per-leaf selectors remain exact owned data after the join carrier is guarded.
use super::*;
use tablecraft_access::reconstruct_saved_read;
use tablecraft_core::values::Value;
use tablecraft_query::{
    QueryError, Source, prepare,
    source::{ExternalSourceRequest, MemorySource, MemoryTable, ResolvedExternalSource, SourceKey},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{
    query::{ReadBody, Statement},
    select::{Table, TableSource},
};

fn source<'a>(table: &'a str, database: Option<&'a str>, alias: Option<&'a str>) -> Fragment<'a> {
    Fragment {
        name1: Some(table),
        expression: database,
        name2: alias,
        ..row(5)
    }
}

fn table(saved: &SavedSelect) -> &Table {
    let Statement::Read(read) = &saved.query().statement else {
        panic!("expected a read");
    };
    let ReadBody::Select(select) = &read.body else {
        panic!("expected SELECT");
    };
    let TableSource::Table(table) = &select.from[0] else {
        panic!("expected a named source");
    };
    table
}

#[test]
fn native_database_tokens_are_copied_exactly_without_identifier_or_sql_interpretation() {
    let long = "x".repeat(1024);
    for token in [
        "remote-token",
        "Remote-TOKEN",
        "C:\\temp\\Database1.accdb",
        "opaque ' é [x]; -- token",
        "] AS injected WHERE False; SELECT 9 --",
        long.as_str(),
    ] {
        let rows = [source("Items", Some(token), Some("r")), column("r.Id")];
        let saved = compile(&rows).unwrap();
        assert_eq!(table(&saved).database.as_deref(), Some(token));
        assert_eq!(table(&saved).name.as_str(), "Items");
        assert_eq!(table(&saved).alias.as_ref().unwrap().as_str(), "r");
        let original = saved
            .definition()
            .rows()
            .iter()
            .find(|row| row.attribute() == Some(tablecraft_ace::QueryAttribute::Table))
            .unwrap();
        assert_eq!(original.expression(), Some(token));
        let serialized = saved.query().to_string();
        assert_eq!(
            parse_query(&serialized, Default::default())
                .unwrap()
                .query(),
            saved.query()
        );
    }
}

#[test]
fn repeated_table_names_attach_selectors_by_verified_tree_leaf_and_alias() {
    let rows = [
        source("Items", Some("one"), Some("a")),
        source("Items", Some("two"), Some("b")),
        source("Local", None, Some("l")),
        joins::join(1, "b", "a", "b.Id=a.Id"),
        joins::join(2, "a", "l", "a.Id=l.Id"),
        column("a.Id"),
    ];
    let saved = compile(&rows).unwrap();
    let expected = parse_query(
        "SELECT a.Id FROM [two].Items b INNER JOIN [one].Items a ON b.Id=a.Id LEFT JOIN Local l ON a.Id=l.Id",
        Default::default(),
    ).unwrap();
    assert_eq!(saved.query(), expected.query());
}

#[test]
fn global_in_and_per_table_selectors_are_separate_owned_fields() {
    let rows = [
        source("Items", Some("leaf"), Some("r")),
        column("r.Id"),
        Fragment {
            flag: Some(2),
            name1: Some("global"),
            ..row(4)
        },
    ];
    let saved = compile(&rows).unwrap();
    let expected = parse_query(
        "SELECT r.Id FROM [leaf].Items r IN 'global'",
        Default::default(),
    )
    .unwrap();
    assert_eq!(saved.query(), expected.query());
}

#[test]
fn invalid_selectors_and_unqualified_native_fields_keep_typed_refusals() {
    for token in ["", "bad\npath", "bad\0path"] {
        assert_eq!(
            compile(&[source("Items", Some(token), None), column("Id")]).err(),
            Some(E::Unsupported)
        );
    }
    for invalid in [
        Fragment {
            flag: Some(1),
            ..source("Items", Some("remote"), None)
        },
        Fragment {
            extra: Some(1),
            ..source("Items", Some("remote"), None)
        },
        source("bad.name", Some("remote"), None),
        source("Items", Some("remote"), Some("bad.alias")),
    ] {
        assert_eq!(
            compile(&[invalid, column("Id")]).err(),
            Some(E::Unsupported)
        );
    }
    assert_eq!(
        compile(&[
            source("Items", Some("one"), Some("r")),
            source("Items", Some("two"), Some("R")),
            column("r.Id"),
        ])
        .err(),
        Some(E::InvalidMetadata)
    );
}

struct Host {
    local: MemorySource,
    target: MemorySource,
    token: String,
    resolutions: Cell<usize>,
}

impl Source for Host {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.local.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.local.scan(table, visitor)
    }

    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        self.resolutions.set(self.resolutions.get() + 1);
        Ok((request.database() == self.token
            && request.connection().is_none()
            && !request.connection_bracketed())
        .then(|| ResolvedExternalSource::new(SourceKey::new(7), &self.target)))
    }
}

fn memory(values: &[i32]) -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            values
                .iter()
                .map(|value| vec![Value::Int32(*value)])
                .collect(),
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn reconstructed_selector_uses_only_the_explicit_host_binding() {
    let token = "opaque ' é ] -- selector";
    let host = Host {
        local: memory(&[999]),
        target: memory(&[7, 8]),
        token: token.into(),
        resolutions: Cell::new(0),
    };
    let saved = compile(&[source("Items", Some(token), Some("r")), column("r.Id")]).unwrap();
    let plan = prepare(saved.query(), &host).unwrap();
    assert_eq!(
        plan.execute(&host, &[], Default::default()).unwrap().rows(),
        [vec![Value::Int32(7)], vec![Value::Int32(8)]]
    );
    assert!(host.resolutions.get() >= 2);
}

#[test]
fn union_arm_fragments_keep_per_table_qualification_inside_their_read_body() {
    let rows = [
        Fragment {
            flag: Some(9),
            ..row(1)
        },
        Fragment {
            expression: Some("SELECT Id FROM [remote].Items"),
            name2: Some("X7YZ_____1"),
            ..row(5)
        },
        Fragment {
            expression: Some("SELECT Id FROM Items"),
            name2: Some("X7YZ_____2"),
            ..row(5)
        },
    ];
    let saved = reconstruct_saved_read(
        definition(&rows, 0x80),
        Default::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = parse_query(
        "SELECT Id FROM [remote].Items UNION ALL SELECT Id FROM Items",
        Default::default(),
    )
    .unwrap();
    assert_eq!(saved.query(), expected.query());
}

#[test]
fn crosstab_selectors_follow_the_shared_verified_leaf_composition() {
    let rows = [
        Fragment {
            flag: Some(6),
            ..row(1)
        },
        source("Items", Some("remote"), Some("r")),
        Fragment {
            flag: Some(2),
            ..column("r.Id")
        },
        Fragment {
            flag: Some(0),
            name1: Some("N"),
            ..column("Count(*)")
        },
        Fragment {
            flag: Some(1),
            ..column("r.Id IN (7,8)")
        },
        Fragment {
            expression: Some("r.Id"),
            flag: Some(2),
            ..row(9)
        },
        Fragment {
            expression: Some("r.Id"),
            flag: Some(1),
            ..row(9)
        },
    ];
    let saved = reconstruct_saved_read(
        definition(&rows, 0x10),
        Default::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let expected = parse_query(
        "TRANSFORM Count(*) AS N SELECT r.Id FROM [remote].Items r GROUP BY r.Id PIVOT r.Id IN (7,8)",
        Default::default(),
    )
    .unwrap();
    assert_eq!(saved.query(), expected.query());
}

#[test]
fn selector_copying_shares_cumulative_bytes_rows_steps_and_cancellation() {
    let rows = [source("Items", Some("x"), None), column("Id")];
    let mut limits = budget();
    let callbacks = Cell::new(0);
    reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut limits,
        &|| {
            callbacks.set(callbacks.get() + 1);
            false
        },
    )
    .unwrap();
    let bytes = budget().remaining(Resource::Bytes) - limits.remaining(Resource::Bytes);
    let rows_used = budget().remaining(Resource::Rows) - limits.remaining(Resource::Rows);
    let steps = budget().remaining(Resource::Steps) - limits.remaining(Resource::Steps);
    reconstruct_saved_select(
        definition(&rows, 0),
        Default::default(),
        &mut ResourceBudget::new(bytes, rows_used, steps),
        &|| false,
    )
    .unwrap();

    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let mut restricted = ResourceBudget::new(
            bytes - u64::from(resource == Resource::Bytes),
            rows_used - u64::from(resource == Resource::Rows),
            steps - u64::from(resource == Resource::Steps),
        );
        assert!(matches!(
            reconstruct_saved_select(
                definition(&rows, 0),
                Default::default(),
                &mut restricted,
                &|| false
            ),
            Err(E::Limit(error)) if error.resource == resource
        ));
    }

    for stop in 1..=callbacks.get() {
        let calls = Cell::new(0);
        let error = reconstruct_saved_select(
            definition(&rows, 0),
            Default::default(),
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        )
        .err();
        assert_eq!(error, Some(E::Cancelled), "{stop}");
    }
}
