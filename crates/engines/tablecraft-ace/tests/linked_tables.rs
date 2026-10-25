#[path = "linked_tables/fixture.rs"]
mod fixture;
mod support;

use std::{
    cell::Cell,
    io::{Cursor, Read, Seek, SeekFrom},
};
use tablecraft_ace::{
    InspectError, NativeLinkText, NativeLinkedTableDefinition, NativeReadSource,
    NativeTableStorage, read_linked_table_definition,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000)
}

fn read(bytes: &[u8]) -> Result<NativeLinkedTableDefinition, InspectError> {
    read_linked_table_definition(
        &mut Cursor::new(bytes),
        CatalogScope::new(7),
        "lInKeD",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn inert_access_definition_preserves_scoped_alias_and_exact_unicode_target() {
    for code in [1, 2, 3, 5, 6] {
        let mut bytes = fixture::database(6, Some("missing/é中.accdb"), Some("Remote table"), None);
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        let original = bytes.clone();
        let result = read(&bytes).unwrap();
        assert_eq!(bytes, original);
        assert_eq!(result.name(), "Linked");
        assert_eq!(result.native_id(), -1);
        assert_eq!(
            result.object_id(),
            CatalogScope::new(7).object_id(u64::from(u32::MAX))
        );
        assert_eq!(result.storage(), NativeTableStorage::Linked);
        assert!(matches!(
            result.database(),
            NativeLinkText::Text("missing/é中.accdb")
        ));
        assert!(matches!(
            result.foreign_name(),
            NativeLinkText::Text("Remote table")
        ));
        assert!(matches!(result.connect(), NativeLinkText::Null));
        let target = result.access_target().unwrap();
        assert_eq!(target.database(), "missing/é中.accdb");
        assert_eq!(target.table(), "Remote table");
    }
}

#[test]
fn opaque_provider_and_missing_values_remain_visible_without_interpretation() {
    for connection in [" ", "ODBC;DSN=not-opened", "Excel 12.0;DATABASE=not-opened"] {
        let bytes = fixture::database(6, Some("missing.file"), Some("SELECT 1"), Some(connection));
        let result = read(&bytes).unwrap();
        assert!(matches!(result.connect(), NativeLinkText::Text(value) if value == connection));
        assert!(matches!(
            result.access_target(),
            Err(InspectError::Unsupported)
        ));
    }
    for (database, table) in [
        (None, Some("Remote")),
        (Some(""), Some("Remote")),
        (Some("missing"), None),
        (Some("missing"), Some("")),
    ] {
        let bytes = fixture::database(6, database, table, None);
        assert!(matches!(
            read(&bytes).unwrap().access_target(),
            Err(InspectError::Unsupported)
        ));
    }
    let bytes = fixture::database(4, Some("missing"), Some("Remote"), None);
    let result = read(&bytes).unwrap();
    assert_eq!(result.storage(), NativeTableStorage::LinkedOdbc);
    assert!(matches!(
        result.access_target(),
        Err(InspectError::Unsupported)
    ));

    let bytes = fixture::database(6, Some(" a;token=opaque "), Some("SELECT 1"), Some(""));
    let result = read(&bytes).unwrap();
    let target = result.access_target().unwrap();
    assert_eq!(target.database(), " a;token=opaque ");
    assert_eq!(target.table(), "SELECT 1");
}

#[test]
fn missing_and_unsupported_optional_declarations_are_distinct_from_null() {
    let original = fixture::database(6, Some("missing"), Some("Remote"), None);
    let mut bytes = original.clone();
    fixture::rename_database(&mut bytes);
    assert!(matches!(
        read(&bytes).unwrap().database(),
        NativeLinkText::Missing
    ));
    assert!(matches!(
        read(&bytes).unwrap().access_target(),
        Err(InspectError::Unsupported)
    ));

    for (column, offset, value) in [(5, 0, 11), (5, 15, 0x92), (6, 16, 1), (7, 0, 9)] {
        let mut bytes = original.clone();
        fixture::set_column_byte(&mut bytes, column, offset, value);
        let result = read(&bytes).unwrap();
        let field = match column {
            5 => result.database(),
            6 => result.connect(),
            _ => result.foreign_name(),
        };
        assert!(matches!(field, NativeLinkText::Unsupported));
        assert!(matches!(
            result.access_target(),
            Err(InspectError::Unsupported)
        ));
    }
}

#[test]
fn local_unknown_orphan_and_duplicate_objects_do_not_become_links() {
    let bytes = fixture::database(1, Some("missing"), Some("Remote"), None);
    assert!(matches!(read(&bytes), Err(InspectError::Unsupported)));
    let bytes = fixture::database(6, Some("missing"), Some("Remote"), None);
    assert!(matches!(
        read_linked_table_definition(
            &mut Cursor::new(&bytes),
            CatalogScope::new(7),
            "missing",
            &mut budget(),
            &|| false
        ),
        Err(InspectError::UnknownTable)
    ));
    assert!(matches!(
        read(&fixture::with_parent(99)),
        Err(InspectError::Malformed)
    ));
    assert!(read(&fixture::with_duplicate_id()).is_err());
}

#[test]
fn complete_metadata_retains_shared_quotas_and_every_observed_cancellation_point() {
    let bytes = fixture::database(6, Some("missing"), Some("Remote"), None);
    let checks = Cell::new(0);
    read_linked_table_definition(
        &mut Cursor::new(&bytes),
        CatalogScope::new(7),
        "Linked",
        &mut budget(),
        &|| {
            checks.set(checks.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in 1..=checks.get() {
        let current = Cell::new(0);
        let result = read_linked_table_definition(
            &mut Cursor::new(&bytes),
            CatalogScope::new(7),
            "Linked",
            &mut budget(),
            &|| {
                current.set(current.get() + 1);
                current.get() >= stop
            },
        );
        assert!(
            matches!(result, Err(InspectError::Cancelled)),
            "checkpoint {stop}"
        );
    }
    for mut allowance in [
        ResourceBudget::new(0, 1000, 100000),
        ResourceBudget::new(1000000, 0, 100000),
        ResourceBudget::new(1000000, 1000, 0),
    ] {
        assert!(
            read_linked_table_definition(
                &mut Cursor::new(&bytes),
                CatalogScope::new(7),
                "Linked",
                &mut allowance,
                &|| false
            )
            .is_err()
        );
    }
}

struct ChangedRow {
    source: Cursor<Vec<u8>>,
    catalog_reads: usize,
    offset: usize,
}

impl Read for ChangedRow {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.source.read(bytes)
    }
}

impl Seek for ChangedRow {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.source.seek(position)
    }
}

impl NativeReadSource for ChangedRow {
    fn decode_native_page(
        &mut self,
        number: u32,
        page: &mut [u8],
        _: &mut ResourceBudget,
        _: &dyn Fn() -> bool,
    ) -> Result<(), InspectError> {
        if number == 4 {
            self.catalog_reads += 1;
            if self.catalog_reads == 3 {
                let start = usize::from(u16::from_le_bytes([page[16], page[17]]) & 0x1fff);
                page[start + self.offset] ^= 1;
            }
        }
        Ok(())
    }
}

#[test]
fn direct_row_must_still_match_captured_id_parent_type_flags_and_name() {
    // Independent fixed-region offsets: Id, ParentId, Type, Flags, then Name.
    for offset in [2, 6, 10, 12, 16] {
        let bytes = fixture::database(6, Some("missing"), Some("Remote"), None);
        let mut source = ChangedRow {
            source: Cursor::new(bytes.clone()),
            catalog_reads: 0,
            offset,
        };
        assert!(matches!(
            read_linked_table_definition(
                &mut source,
                CatalogScope::new(7),
                "Linked",
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Malformed)
        ));
        assert_eq!(source.source.into_inner(), bytes);
        assert_eq!(source.catalog_reads, 3);
    }
}

#[test]
fn selected_malformed_memo_fails_while_unqualified_payload_is_never_decoded() {
    let bytes = fixture::malformed_database();
    assert!(matches!(read(&bytes), Err(InspectError::Malformed)));
    let mut bytes = bytes;
    fixture::set_column_byte(&mut bytes, 5, 0, 11);
    let result = read(&bytes).unwrap();
    assert!(matches!(result.database(), NativeLinkText::Unsupported));
    assert!(matches!(
        result.foreign_name(),
        NativeLinkText::Text("Remote")
    ));
    assert!(matches!(
        result.access_target(),
        Err(InspectError::Unsupported)
    ));
}

#[test]
fn repeated_inspections_share_consumed_native_budgets_without_refill() {
    let bytes = fixture::database(6, Some("missing"), Some("Remote"), None);
    let mut measured = budget();
    read_linked_table_definition(
        &mut Cursor::new(&bytes),
        CatalogScope::new(7),
        "Linked",
        &mut measured,
        &|| false,
    )
    .unwrap();
    for resource in [Resource::Bytes, Resource::Rows, Resource::Steps] {
        let used = budget().remaining(resource) - measured.remaining(resource);
        let ceiling = used + used / 2;
        let mut shared = ResourceBudget::new(
            if resource == Resource::Bytes {
                ceiling
            } else {
                16 * 1024 * 1024
            },
            if resource == Resource::Rows {
                ceiling
            } else {
                16_384
            },
            if resource == Resource::Steps {
                ceiling
            } else {
                1_000_000
            },
        );
        read_linked_table_definition(
            &mut Cursor::new(&bytes),
            CatalogScope::new(7),
            "Linked",
            &mut shared,
            &|| false,
        )
        .unwrap();
        let result = read_linked_table_definition(
            &mut Cursor::new(&bytes),
            CatalogScope::new(7),
            "Linked",
            &mut shared,
            &|| false,
        );
        assert!(matches!(result, Err(InspectError::Limit(error)) if error.resource == resource));
    }
}
