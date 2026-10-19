#[allow(dead_code)]
#[path = "../../../engines/tablecraft-ace/tests/mutation/fixture.rs"]
mod fixture;
#[path = "../../../engines/tablecraft-ace/tests/support/mod.rs"]
mod support;
use std::io::Cursor;
use tablecraft_access::{NativeBrowser, TablePageRequest};
use tablecraft_ace::InspectError;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[test]
fn library_inventory_and_pages_are_usable_without_cli_or_query_evaluation() {
    let mut bytes = fixture::database();
    const PAGE_BYTES: usize = 4096;
    const CATALOG_DEFINITION_PAGE: usize = 2;
    const RECORD_COUNT_OFFSET: usize = 16;
    const CATALOG_DATA_PAGE: usize = 4;
    support::put32(
        &mut bytes,
        CATALOG_DEFINITION_PAGE * PAGE_BYTES + RECORD_COUNT_OFFSET,
        4,
    );
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        support::record(10, 0, 1, 0, b"\xff\xfeItems"),
        support::record(42, 0, 5, 0, b"\xff\xfeSaved query"),
    ];
    bytes[CATALOG_DATA_PAGE * PAGE_BYTES..(CATALOG_DATA_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &support::data_page(
            2,
            &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
        ),
    );
    let original = bytes.clone();
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &|| false,
    );
    let inventory = browser.tables().unwrap();
    assert_eq!(
        inventory
            .tables()
            .map(|t| t.name().as_str())
            .collect::<Vec<_>>(),
        ["MSysObjects", "MSysRelationships", "Items"]
    );
    let page = browser
        .page("Items", TablePageRequest::new(1, 1).unwrap())
        .unwrap();
    assert_eq!(page.rows(), &[vec![Value::Int32(2), Value::Int32(20)]]);
    let end = browser
        .page("Items", TablePageRequest::new(3, 1).unwrap())
        .unwrap();
    assert!(end.rows().is_empty());
    assert!(end.reached_end());
    assert_eq!(bytes, original);
}
#[test]
fn operations_share_budget_and_cancellation_instead_of_resetting_controls() {
    let bytes = fixture::database();
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 4, 1_000_000),
        &|| false,
    );
    browser.tables().unwrap();
    assert!(matches!(
        browser.page("Items", TablePageRequest::new(0, 1).unwrap()),
        Err(InspectError::Limit(_))
    ));
    let mut cancelled = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &|| true,
    );
    assert!(matches!(cancelled.tables(), Err(InspectError::Cancelled)));
    assert!(matches!(
        cancelled.page("Items", TablePageRequest::new(0, 1).unwrap()),
        Err(InspectError::Cancelled)
    ));
}

#[path = "../../../engines/tablecraft-ace/tests/support/page_reads.rs"]
mod page_reads;

fn with_linked_tables() -> Vec<u8> {
    const PAGE_BYTES: usize = 4096;
    let mut bytes = fixture::database();
    support::put32(&mut bytes, 2 * PAGE_BYTES + 16, 5);
    let objects = [
        support::record(2, 0, 1, 0x80000002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x80000002, b"\xff\xfeMSysRelationships"),
        support::record(10, 0, 1, 0, b"\xff\xfeItems"),
        // These IDs cannot be local definition pages in this small fixture.
        support::record(9000, 0, 6, 0, b"\xff\xfeRemoteAccess"),
        support::record(9001, 0, 4, 0, b"\xff\xfeRemoteOdbc"),
    ];
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&support::data_page(
        2,
        &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
    ));
    bytes
}
#[test]
fn inventory_exposes_storage_and_never_follows_linked_definitions() {
    use tablecraft_access::NativeTableStorage as Storage;
    let bytes = with_linked_tables();
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &|| false,
    );
    let inventory = browser.tables().unwrap();
    assert_eq!(
        inventory
            .entries()
            .map(|entry| (entry.object().name().as_str(), entry.storage()))
            .collect::<Vec<_>>(),
        [
            ("MSysObjects", Some(Storage::Local)),
            ("MSysRelationships", Some(Storage::Local)),
            ("Items", Some(Storage::Local)),
            ("RemoteAccess", Some(Storage::Linked)),
            ("RemoteOdbc", Some(Storage::LinkedOdbc)),
        ]
    );
    for table in ["RemoteAccess", "RemoteOdbc"] {
        assert!(matches!(
            browser.describe(table),
            Err(InspectError::Unsupported)
        ));
        assert!(matches!(
            browser.page(table, TablePageRequest::new(0, 1).unwrap()),
            Err(InspectError::Unsupported)
        ));
    }
}
#[test]
fn schema_reports_carriers_without_reading_or_qualifying_data_pages() {
    const PAGE_BYTES: usize = 4096;
    const DATA_PAGE: usize = 12;
    const DEFINITION_PAGE: usize = 10;
    const COLUMN_OFFSET: usize = 63;
    let mut bytes = fixture::database();
    bytes[DATA_PAGE * PAGE_BYTES] = 0;
    let mut input = page_reads::PageReads::new(bytes.clone());
    {
        let mut browser = NativeBrowser::new(
            &mut input,
            CatalogScope::new(1),
            ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
            &|| false,
        );
        let description = browser.describe("Items").unwrap();
        assert!(description.scalar_columns_supported());
        assert_eq!(description.unsupported_columns().count(), 0);
        assert_eq!(description.schema().declared_rows(), 3);
    }
    assert_eq!(input.count(DATA_PAGE as u64), 0);
    let mut browser = NativeBrowser::new(
        &mut input,
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &|| false,
    );
    assert!(matches!(
        browser.page("Items", TablePageRequest::new(0, 1).unwrap()),
        Err(InspectError::Malformed)
    ));
    bytes[DEFINITION_PAGE * PAGE_BYTES + COLUMN_OFFSET] = u8::MAX;
    let mut browser = NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
        &|| false,
    );
    let description = browser.describe("Items").unwrap();
    assert!(!description.scalar_columns_supported());
    assert_eq!(
        description
            .unsupported_columns()
            .map(|c| c.name())
            .collect::<Vec<_>>(),
        ["Id"]
    );
}
#[test]
fn describe_propagates_unknown_malformed_limits_and_final_cancellation() {
    use std::cell::Cell;
    let bytes = fixture::database();
    let budget = || ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000);
    let mut missing =
        NativeBrowser::new(Cursor::new(&bytes), CatalogScope::new(1), budget(), &|| {
            false
        });
    assert!(matches!(
        missing.describe("Missing"),
        Err(InspectError::UnknownTable)
    ));
    let calls = Cell::new(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        false
    };
    NativeBrowser::new(
        Cursor::new(&bytes),
        CatalogScope::new(1),
        budget(),
        &cancelled,
    )
    .describe("Items")
    .unwrap();
    let final_call = calls.get();
    calls.set(0);
    let cancelled = || {
        calls.set(calls.get() + 1);
        calls.get() == final_call
    };
    assert!(matches!(
        NativeBrowser::new(
            Cursor::new(&bytes),
            CatalogScope::new(1),
            budget(),
            &cancelled
        )
        .describe("Items"),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        NativeBrowser::new(
            Cursor::new(&bytes),
            CatalogScope::new(1),
            ResourceBudget::new(0, 0, 0),
            &|| false
        )
        .describe("Items"),
        Err(InspectError::Limit(_))
    ));
    let mut malformed = bytes.clone();
    malformed[10 * 4096] = 0;
    assert!(matches!(
        NativeBrowser::new(
            Cursor::new(malformed),
            CatalogScope::new(1),
            budget(),
            &|| false
        )
        .describe("Items"),
        Err(InspectError::Malformed)
    ));
}
