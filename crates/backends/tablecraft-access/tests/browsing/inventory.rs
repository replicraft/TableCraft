//! Actual catalog-only discovery with malformed unrequested bodies left inert.
use std::io::Cursor;
use tablecraft_access::NativeBrowser;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};
use tablecraft_testkit::{fixtures::access::saved_queries, support::ace::page_reads::PageReads};

#[test]
fn object_inventory_preserves_full_ids_and_never_opens_query_or_user_records() {
    const PAGE: usize = 4096;
    let mut bytes = saved_queries::database();
    // Query fragment framing and user records are irrelevant to enumeration.
    bytes[14 * PAGE..15 * PAGE].fill(0xcc);
    bytes[16 * PAGE..17 * PAGE].fill(0xdd);
    bytes[12 * PAGE..13 * PAGE].fill(0xee);
    let mut source = PageReads::new(bytes);
    let objects = {
        let mut browser = NativeBrowser::new(
            &mut source,
            CatalogScope::new(0x123456789abcdef0),
            ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
            &|| false,
        );
        browser.inventory().unwrap()
    };
    let catalog_reads = source.count(4);
    assert!(catalog_reads > 0);
    let queries = objects
        .queries()
        .map(|entry| {
            (
                entry.object().id().as_u128(),
                entry.object().name().as_str(),
                entry.object().kind(),
                entry.is_system(),
                entry.storage(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        queries,
        [
            (
                0x123456789abcdef0_000000008000007b,
                "Saved",
                ObjectKind::Query,
                true,
                None
            ),
            (
                0x123456789abcdef0_000000008000007c,
                "Empty",
                ObjectKind::Query,
                false,
                None
            ),
        ]
    );
    assert_eq!(objects.entries().count(), 6);
    assert_eq!(objects.tables().count(), 4);
    assert_eq!(source.count(4), catalog_reads);
    for page in [10, 12, 14, 16] {
        assert_eq!(
            source.count(page),
            0,
            "unrequested native object page {page}"
        );
    }
}

#[test]
fn inventory_ids_remain_source_scoped_even_for_the_same_original_bytes() {
    let bytes = saved_queries::database();
    let ids = [1, 2].map(|scope| {
        let mut browser = NativeBrowser::new(
            Cursor::new(&bytes),
            CatalogScope::new(scope),
            ResourceBudget::new(16 * 1024 * 1024, 100, 1_000_000),
            &|| false,
        );
        let objects = browser.inventory().unwrap();
        objects
            .queries()
            .map(|entry| entry.object().id().as_u128())
            .collect::<Vec<_>>()
    });
    assert_eq!(ids[0], [0x1_000000008000007b, 0x1_000000008000007c]);
    assert_eq!(ids[1], [0x2_000000008000007b, 0x2_000000008000007c]);
}
