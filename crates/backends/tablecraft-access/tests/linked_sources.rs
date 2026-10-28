#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
use tablecraft_testkit::access as support;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod target_fixture;

#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_links/fixture.rs"]
mod collection_fixture;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "linked_sources/forwarding.rs"]
mod forwarding;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/version_history/fixture.rs"]
mod history_fixture;
#[path = "linked_sources/rejections.rs"]
mod rejections;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_access::{LinkSourceError, LinkedNativeSource, NativeLinkMapping, NativeSource};
use tablecraft_ace::NativeReadSource;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare};
use tablecraft_sql::parse_query;
use target_fixture as base;

const PAGE_BYTES: u64 = 4096;
const TARGET_ROW_PAGE: u64 = 12;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ROWS: u64 = 16_384;
const MAX_STEPS: u64 = 1_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(MAX_BYTES, MAX_ROWS, MAX_STEPS)
}

fn root_bytes() -> Vec<u8> {
    linked_fixture::database(6, Some("opaque-backend"), Some("Items"), None)
}

fn mappings<'a>(target: &'a NativeSource<'a, impl NativeReadSource>) -> [NativeLinkMapping<'a>; 1] {
    [NativeLinkMapping::new(
        "Linked",
        -1,
        "opaque-backend",
        target.handle(),
        "Items",
        10,
    )]
}

fn execute(source: &impl Source, sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    let result =
        prepare(parsed.query(), source)?.execute(source, &[], ExecutionOptions::default())?;
    Ok(result.rows().to_vec())
}

struct Watching<'a> {
    inner: Cursor<Vec<u8>>,
    rows_forbidden: &'a Cell<bool>,
    fail_rows: &'a Cell<bool>,
}

impl NativeReadSource for Watching<'_> {}

impl Read for Watching<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.inner.position() / PAGE_BYTES == TARGET_ROW_PAGE {
            assert!(!self.rows_forbidden.get(), "binding read target user rows");
            if self.fail_rows.get() {
                return Err(std::io::Error::other("injected target row failure"));
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
fn mixed_handle_types_bind_without_rows_then_execute_exact_target_values() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let forbidden = Cell::new(true);
    let fail = Cell::new(false);
    let target = NativeSource::new_with_shared_budget(
        Watching {
            inner: Cursor::new(target_fixture::database()),
            rows_forbidden: &forbidden,
            fail_rows: &fail,
        },
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
    assert!(facade.columns("linked").unwrap() == target.columns("Items").unwrap());
    assert_eq!(
        facade.text_collations("Linked").unwrap(),
        target.text_collations("Items").unwrap()
    );
    let parsed = parse_query("SELECT Score FROM Linked ORDER BY Id", Default::default()).unwrap();
    let prepared = prepare(parsed.query(), &facade).unwrap();
    forbidden.set(false);
    assert_eq!(
        prepared
            .execute(&facade, &[], ExecutionOptions::default())
            .unwrap()
            .rows(),
        [
            vec![Value::Int32(10)],
            vec![Value::Int32(20)],
            vec![Value::Int32(30)],
        ]
    );
    assert!(facade.query("Items").unwrap().is_none());
    assert!(matches!(
        facade.columns("Items"),
        Err(QueryError::UnknownName)
    ));
}

#[test]
fn unmapped_links_stay_inert_until_accessed_and_local_root_metadata_stays_available() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = LinkedNativeSource::new(root.handle(), &[]).unwrap();
    assert!(!facade.columns("MSysObjects").unwrap().is_empty());
    assert!(matches!(
        facade.columns("Linked"),
        Err(QueryError::Unsupported)
    ));
    assert!(matches!(
        facade.text_equality_hints("Linked"),
        Err(QueryError::Unsupported)
    ));
    assert!(matches!(
        execute(&facade, "SELECT * FROM Linked"),
        Err(QueryError::Unsupported)
    ));
}

#[test]
fn expected_identifiers_and_native_ids_are_exact_not_provider_or_path_resolution() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    for (alias, id, database, table, target_id) in [
        ("linked", -1, "opaque-backend", "Items", 10),
        ("Linked", 1, "opaque-backend", "Items", 10),
        ("Linked", -1, "opaque-backend ", "Items", 10),
        ("Linked", -1, "opaque-backend", "items", 10),
        ("Linked", -1, "opaque-backend", "Items", 11),
    ] {
        let mapping =
            NativeLinkMapping::new(alias, id, database, target.handle(), table, target_id);
        assert!(matches!(
            LinkedNativeSource::new(root.handle(), &[mapping]),
            Err(LinkSourceError::IdentityMismatch)
        ));
    }
    let extra = NativeLinkMapping::new(
        "MSysObjects",
        2,
        "opaque-backend",
        target.handle(),
        "Items",
        10,
    );
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &[extra]),
        Err(LinkSourceError::ExtraMapping)
    ));
    let [first] = mappings(&target);
    let [second] = mappings(&target);
    let duplicate = [first, second];
    assert!(matches!(
        LinkedNativeSource::new(root.handle(), &duplicate),
        Err(LinkSourceError::DuplicateMapping)
    ));
}

#[test]
fn late_target_failure_never_publishes_a_complete_cache_and_visitor_rejection_propagates() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let forbidden = Cell::new(false);
    let fail = Cell::new(true);
    let target = NativeSource::new_with_shared_budget(
        Watching {
            inner: Cursor::new(target_fixture::database()),
            rows_forbidden: &forbidden,
            fail_rows: &fail,
        },
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
    let before = pool.borrow().remaining(Resource::Bytes);
    assert!(matches!(
        execute(&facade, "SELECT Score FROM Linked"),
        Err(QueryError::Source)
    ));
    assert!(pool.borrow().remaining(Resource::Bytes) < before);
    fail.set(false);
    let mut visited = 0;
    assert_eq!(
        facade.scan("Linked", &mut |_| {
            visited += 1;
            Err(QueryError::Cancelled)
        }),
        Err(QueryError::Cancelled)
    );
    assert_eq!(visited, 1);
    assert_eq!(
        execute(&facade, "SELECT Count(*) FROM Linked").unwrap(),
        [vec![Value::Int64(3)]]
    );
}

#[test]
fn exact_stored_foreign_name_resolves_under_existing_source_name_policy() {
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("opaque-backend"),
            Some("items"),
            None,
        )),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(target_fixture::database()),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap();
    let mapping =
        NativeLinkMapping::new("Linked", -1, "opaque-backend", target.handle(), "items", 10);
    let facade = LinkedNativeSource::new(root.handle(), &[mapping]).unwrap();
    assert_eq!(
        execute(&facade, "SELECT Count(*) FROM Linked").unwrap(),
        [vec![Value::Int64(3)]]
    );
}
