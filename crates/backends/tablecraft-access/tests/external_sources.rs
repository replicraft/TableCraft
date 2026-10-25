#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;

#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod collection_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/version_history/fixture.rs"]
mod history_fixture;
#[path = "external_sources/named.rs"]
mod named;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/saved_queries/fixture.rs"]
mod saved_fixture;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_access::{
    ExternalNativeSource, ExternalSourceError, LinkSourceError, LinkedNativeSource,
    NativeExternalMapping, NativeLinkMapping, NativeSource,
};
use tablecraft_ace::NativeReadSource;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{
    ExecutionOptions, QueryError, Source, prepare,
    source::{ExternalSourceRequest, SourceKey},
};
use tablecraft_sql::parse_query;

const PAGE_BYTES: u64 = 4096;
const ROW_PAGE: u64 = 12;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

fn allowance() -> ResourceBudget {
    ResourceBudget::new(MAX_BYTES, 16_384, 1_000_000)
}

fn distinct_target_bytes() -> Vec<u8> {
    let mut bytes = base::database();
    let rows = [(1_i32, 111_i32), (2, 222), (3, 333)]
        .into_iter()
        .map(|(id, score)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(score.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    bytes
}

fn mapping<'a>(target: &'a NativeSource<'_, impl NativeReadSource>) -> NativeExternalMapping<'a> {
    NativeExternalMapping::new("remote", SourceKey::new(42), target.handle())
}

fn request(database: &str) -> ExternalSourceRequest<'_> {
    ExternalSourceRequest::new(database, None, false)
}

fn scores(source: &impl Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(source, &[], ExecutionOptions::default())?
        .rows()
        .to_vec())
}

struct Watching<'a> {
    inner: Cursor<Vec<u8>>,
    forbid_rows: &'a Cell<bool>,
    fail_rows: &'a Cell<bool>,
}

impl NativeReadSource for Watching<'_> {}

impl Read for Watching<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.inner.position() / PAGE_BYTES == ROW_PAGE {
            assert!(
                !self.forbid_rows.get(),
                "metadata preparation read user rows"
            );
            if self.fail_rows.get() {
                return Err(std::io::Error::other("injected row failure"));
            }
        }
        self.inner.read(bytes)
    }
}

impl Seek for Watching<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(position)
    }
}

#[test]
fn exact_tokens_prepare_without_rows_then_read_the_explicit_target() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let forbidden = Cell::new(true);
    let failed = Cell::new(false);
    let target = NativeSource::new_with_shared_budget(
        Watching {
            inner: Cursor::new(distinct_target_bytes()),
            forbid_rows: &forbidden,
            fail_rows: &failed,
        },
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    assert!(facade.external_source(request("Remote")).unwrap().is_none());
    let remote = facade.external_source(request("remote")).unwrap().unwrap();
    assert!(remote.key() == SourceKey::new(42));
    assert!(remote.source().columns("Items").unwrap() == target.columns("Items").unwrap());
    assert!(remote.source().query("Items").unwrap().is_none());
    assert!(
        remote
            .source()
            .external_source(request("remote"))
            .unwrap()
            .is_none()
    );
    let parsed = parse_query(
        "SELECT Score FROM Items IN 'remote' ORDER BY Id",
        Default::default(),
    )
    .unwrap();
    let prepared = prepare(parsed.query(), &facade).unwrap();
    forbidden.set(false);
    assert_eq!(
        prepared
            .execute(&facade, &[], Default::default())
            .unwrap()
            .rows(),
        [
            vec![Value::Int32(111)],
            vec![Value::Int32(222)],
            vec![Value::Int32(333)],
        ]
    );
}

#[test]
fn connection_forms_and_nonlocal_targets_do_not_activate_providers() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("anything"),
            Some("Items"),
            None,
        )),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    for input in [
        ExternalSourceRequest::new("remote", Some(""), false),
        ExternalSourceRequest::new("remote", Some("Access"), false),
        ExternalSourceRequest::new("remote", None, true),
    ] {
        assert!(matches!(
            facade.external_source(input),
            Err(QueryError::Unsupported)
        ));
    }
    let external = facade.external_source(request("remote")).unwrap().unwrap();
    assert!(matches!(
        external.source().columns("Linked"),
        Err(QueryError::Unsupported)
    ));
    assert!(matches!(
        external.source().columns("Unknown"),
        Err(QueryError::UnknownName)
    ));
}

#[test]
fn mappings_require_shared_pool_distinct_scopes_and_unambiguous_keys() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let owned = NativeSource::new(
        Cursor::new(base::database()),
        CatalogScope::new(2),
        allowance(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        ExternalNativeSource::new(root.handle(), &[mapping(&owned)]),
        Err(ExternalSourceError::Identity(
            LinkSourceError::SharedBudgetRequired
        ))
    ));
    let collision = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        ExternalNativeSource::new(root.handle(), &[mapping(&collision)]),
        Err(ExternalSourceError::Identity(
            LinkSourceError::ScopeCollision
        ))
    ));
    let a = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let b = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(3),
        &pool,
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        ExternalNativeSource::new(root.handle(), &[mapping(&a), mapping(&a)]),
        Err(ExternalSourceError::DuplicateToken)
    ));
    assert!(matches!(
        ExternalNativeSource::new(
            root.handle(),
            &[
                mapping(&a),
                NativeExternalMapping::new("other", SourceKey::new(42), b.handle()),
            ]
        ),
        Err(ExternalSourceError::DuplicateKey)
    ));
}

#[test]
fn root_link_routes_and_external_base_tables_keep_separate_namespaces() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("backend"),
            Some("Items"),
            None,
        )),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let links = LinkedNativeSource::new(
        root.handle(),
        &[NativeLinkMapping::new(
            "Linked",
            -1,
            "backend",
            target.handle(),
            "Items",
            10,
        )],
    )
    .unwrap();
    let facade = ExternalNativeSource::with_links(&links, &[mapping(&target)]).unwrap();
    assert_eq!(
        scores(&facade, "SELECT Score FROM Linked ORDER BY Id").unwrap(),
        scores(&facade, "SELECT Score FROM Items IN 'remote' ORDER BY Id").unwrap()
    );
    assert!(matches!(
        facade.columns("Items"),
        Err(QueryError::UnknownName)
    ));
    let remote = facade.external_source(request("remote")).unwrap().unwrap();
    assert!(matches!(
        remote.source().columns("Linked"),
        Err(QueryError::UnknownName)
    ));
    let same_scope = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        ExternalNativeSource::with_links(&links, &[mapping(&same_scope)]),
        Err(ExternalSourceError::Identity(
            LinkSourceError::ScopeCollision
        ))
    ));
}

#[test]
fn cancellation_borrow_collisions_and_exhaustion_fail_without_refills() {
    let pool = RefCell::new(allowance());
    let cancelled = Cell::new(false);
    let cancel = || cancelled.get();
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(1),
        &pool,
        &cancel,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    let held = pool.borrow_mut();
    assert!(matches!(
        facade.external_source(request("remote")),
        Err(QueryError::Source)
    ));
    drop(held);
    cancelled.set(true);
    assert!(matches!(
        facade.external_source(request("remote")),
        Err(QueryError::Cancelled)
    ));
    cancelled.set(false);
    let remaining = pool.borrow().remaining(Resource::Steps);
    pool.borrow_mut()
        .charge(Resource::Steps, remaining)
        .unwrap();
    assert!(matches!(
        facade.external_source(request("remote")),
        Err(QueryError::Limit(_))
    ));
    assert_eq!(pool.borrow().remaining(Resource::Steps), 0);
}

#[test]
fn failed_target_rows_keep_shared_native_charges_and_publish_no_result() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let forbidden = Cell::new(false);
    let failed = Cell::new(true);
    let target = NativeSource::new_with_shared_budget(
        Watching {
            inner: Cursor::new(base::database()),
            forbid_rows: &forbidden,
            fail_rows: &failed,
        },
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
    let before = pool.borrow().remaining(Resource::Steps);
    assert!(scores(&facade, "SELECT Score FROM Items IN 'remote'").is_err());
    assert!(pool.borrow().remaining(Resource::Steps) < before);
}

#[test]
fn exact_root_handle_can_be_reused_without_authorizing_link_recursion() {
    let pool = RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(base::database()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = ExternalNativeSource::new(root.handle(), &[mapping(&root)]).unwrap();
    assert_eq!(
        scores(&facade, "SELECT Score FROM Items IN 'remote' ORDER BY Id").unwrap(),
        scores(&facade, "SELECT Score FROM Items ORDER BY Id").unwrap(),
    );

    let linked = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("backend"),
            Some("Items"),
            None,
        )),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let links = LinkedNativeSource::new(linked.handle(), &[]).unwrap();
    let facade = ExternalNativeSource::with_links(&links, &[mapping(&linked)]).unwrap();
    let external = facade.external_source(request("remote")).unwrap().unwrap();
    assert!(matches!(
        external.source().columns("Linked"),
        Err(QueryError::Unsupported)
    ));
}

#[test]
fn external_targets_preserve_collection_metadata_policies_and_aligned_rows() {
    for (bytes, sql, expected) in [
        (
            collection_fixture::populated(),
            "SELECT Id,Score.Value FROM Items IN 'remote' ORDER BY Id,Score.Value",
            vec![
                vec![Value::Int32(1), Value::Int32(100)],
                vec![Value::Int32(1), Value::Int32(101)],
                vec![Value::Int32(2), Value::Int32(200)],
                vec![Value::Int32(3), Value::Null],
            ],
        ),
        (
            history_fixture::database(),
            "SELECT Id FROM Items IN 'remote' WHERE Score IS NOT NULL ORDER BY Id",
            vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
        ),
    ] {
        let pool = RefCell::new(allowance());
        let root = NativeSource::new_with_shared_budget(
            Cursor::new(base::database()),
            CatalogScope::new(1),
            &pool,
            &|| false,
        )
        .unwrap();
        let target = NativeSource::new_with_shared_budget(
            Cursor::new(bytes),
            CatalogScope::new(2),
            &pool,
            &|| false,
        )
        .unwrap()
        .with_collections();
        let facade = ExternalNativeSource::new(root.handle(), &[mapping(&target)]).unwrap();
        let external = facade.external_source(request("remote")).unwrap().unwrap();
        let remote = external.source();

        assert!(
            remote.unavailable_columns("Items").unwrap()
                == target.unavailable_columns("Items").unwrap()
        );
        assert!(
            remote.multivalue_columns("Items").unwrap()
                == target.multivalue_columns("Items").unwrap()
        );
        assert_eq!(
            remote.multivalue_text_collations("Items").unwrap(),
            target.multivalue_text_collations("Items").unwrap()
        );
        assert!(
            remote.collection_members("Items").unwrap()
                == target.collection_members("Items").unwrap()
        );
        assert_eq!(
            remote.collection_member_text_collations("Items").unwrap(),
            target.collection_member_text_collations("Items").unwrap()
        );
        assert_eq!(scores(&facade, sql).unwrap(), expected);
    }
}

#[path = "external_sources/progressive.rs"]
mod progressive;
