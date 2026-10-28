//! Inert borrowed identities over original synthetic native fixtures.
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/linked_tables/fixture.rs"]
mod linked_fixture;
use tablecraft_testkit::access as support;
#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod target_fixture;

use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_access::{NativeSource, NativeTableStorage};
use tablecraft_ace::{InspectError, NativeReadSource};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;
use tablecraft_query::QueryError;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

fn remaining(pool: &RefCell<ResourceBudget>) -> [u64; 3] {
    let quota = pool.borrow();
    [
        quota.remaining(Resource::Bytes),
        quota.remaining(Resource::Rows),
        quota.remaining(Resource::Steps),
    ]
}

struct Watching<'a> {
    bytes: Cursor<Vec<u8>>,
    reads: &'a Cell<usize>,
    forbidden: &'a Cell<bool>,
}

impl NativeReadSource for Watching<'_> {}

impl Read for Watching<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        assert!(
            !self.forbidden.get(),
            "identity lookup performed native I/O"
        );
        self.reads.set(self.reads.get() + 1);
        self.bytes.read(bytes)
    }
}

impl Seek for Watching<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn owned_and_shared_readers_erase_to_borrowed_canonical_scoped_identities_without_io() {
    let owned_scope = CatalogScope::new(11);
    let owned = NativeSource::new(
        Cursor::new(target_fixture::database()),
        owned_scope,
        budget(),
        &|| false,
    )
    .unwrap();
    let shared_scope = CatalogScope::new(12);
    let pool = RefCell::new(budget());
    let reads = Cell::new(0);
    let forbidden = Cell::new(false);
    let shared = NativeSource::new_with_shared_budget(
        Watching {
            bytes: Cursor::new(target_fixture::database()),
            reads: &reads,
            forbidden: &forbidden,
        },
        shared_scope,
        &pool,
        &|| false,
    )
    .unwrap();
    let before = remaining(&pool);
    let loaded = reads.get();
    forbidden.set(true);
    let handles = [owned.handle(), shared.handle()];
    for (handle, scope) in handles.into_iter().zip([owned_scope, shared_scope]) {
        assert_eq!(handle.scope(), scope);
        let identity = handle.table_identity("iTeMs").unwrap();
        assert_eq!(identity.name(), "Items");
        assert_eq!(identity.native_id(), 10);
        assert_eq!(identity.object_id(), scope.object_id(10));
        assert_eq!(identity.storage(), NativeTableStorage::Local);
    }
    assert_ne!(
        owned.handle().table_identity("Items").unwrap().object_id(),
        shared.handle().table_identity("Items").unwrap().object_id()
    );
    assert_eq!(reads.get(), loaded);
    assert_eq!(remaining(&pool), before);
}

#[test]
fn captured_storage_is_not_inferred_from_names_or_treated_as_readability() {
    for (kind, expected) in [
        (1, Some(NativeTableStorage::Local)),
        (6, Some(NativeTableStorage::Linked)),
        (4, Some(NativeTableStorage::LinkedOdbc)),
        (99, None),
    ] {
        let source = NativeSource::new(
            Cursor::new(linked_fixture::database(
                kind,
                Some("not-opened"),
                Some("Remote"),
                None,
            )),
            CatalogScope::new(13),
            budget(),
            &|| false,
        )
        .unwrap();
        let identity = source.handle().table_identity("Linked");
        if let Some(storage) = expected {
            let identity = identity.unwrap();
            assert_eq!(identity.storage(), storage);
            assert_eq!(identity.native_id(), -1);
            assert_eq!(
                identity.object_id(),
                CatalogScope::new(13).object_id(u64::from(u32::MAX))
            );
        } else {
            assert_eq!(identity.err(), Some(QueryError::UnknownName));
        }
        assert_eq!(
            source.handle().table_identity("Missing").err(),
            Some(QueryError::UnknownName)
        );
    }
}

#[test]
fn explicit_definition_reads_keep_the_same_pool_and_typed_control_failures() {
    let pool = RefCell::new(budget());
    let cancelled = Cell::new(false);
    let cancellation = || cancelled.get();
    let source = NativeSource::new_with_shared_budget(
        Cursor::new(linked_fixture::database(
            6,
            Some("missing/opaque.accdb"),
            Some("Remote"),
            None,
        )),
        CatalogScope::new(14),
        &pool,
        &cancellation,
    )
    .unwrap();
    let before = remaining(&pool);
    let definition = source
        .handle()
        .read_linked_table_definition("lInKeD")
        .unwrap();
    assert_eq!(definition.name(), "Linked");
    assert_eq!(definition.native_id(), -1);
    assert_eq!(
        definition.object_id(),
        source
            .handle()
            .table_identity("Linked")
            .unwrap()
            .object_id()
    );
    assert_eq!(
        definition.access_target().unwrap().database(),
        "missing/opaque.accdb"
    );
    let read = remaining(&pool);
    assert!(read[0] < before[0]);
    assert!(read[2] < before[2]);

    cancelled.set(true);
    assert!(matches!(
        source.handle().read_linked_table_definition("Linked"),
        Err(InspectError::Cancelled)
    ));
    assert_eq!(
        source.handle().table_identity("Linked").err(),
        Some(QueryError::Cancelled)
    );
    assert_eq!(remaining(&pool), read);

    cancelled.set(false);
    let held = pool.borrow_mut();
    assert!(matches!(
        source.handle().read_linked_table_definition("Linked"),
        Err(InspectError::Unsupported)
    ));
    drop(held);
    assert_eq!(remaining(&pool), read);

    let steps = pool.borrow().remaining(Resource::Steps);
    pool.borrow_mut().charge(Resource::Steps, steps).unwrap();
    assert!(matches!(
        source.handle().read_linked_table_definition("Linked"),
        Err(InspectError::Limit(limit)) if limit.resource == Resource::Steps
    ));
    assert_eq!(pool.borrow().remaining(Resource::Steps), 0);
}
