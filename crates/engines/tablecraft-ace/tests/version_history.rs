#[path = "mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "version_history/compression.rs"]
mod compression;
#[path = "version_history/fixture.rs"]
mod fixture;
mod support;
#[path = "version_history/visible_memo.rs"]
mod visible_memo;

use fixture::{database, database_with, definition};
use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use support::put32;
use tablecraft_ace::{
    CollectionPageRequest, InspectError, StoredCollectionColumn, TablePageRequest,
    TableSnapshotLimits, VersionHistoryColumn, read_stored_collection_table_page_with_request,
    read_version_history_column,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const FIELD_START: usize = 63;
const FIELD_BYTES: usize = 25;
const TEMPLATE_PAGE: usize = 17;
const DATA_PAGE: usize = 22;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8], memo: &str) -> Result<VersionHistoryColumn, InspectError> {
    read_version_history_column(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        memo,
        &mut budget(),
        &|| false,
    )
}

#[test]
fn visible_memo_resolves_hidden_history_without_sorting_replacing_or_synthesizing_values() {
    for code in [2, 5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        let history = read(&bytes, "nOtEs").unwrap();
        assert_eq!(history.memo_column().name(), "Notes");
        assert_eq!(history.links().column().number(), 1);
        let values = history
            .stored_versions(0)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            values.iter().map(|v| v.identity()).collect::<Vec<_>>(),
            [2, 9]
        );
        assert_eq!(values[0].value(), Some("é🦀\0older"));
        assert_eq!(values[1].value(), Some("newer"));
        assert_eq!(
            values[0].modified().unwrap().to_bits(),
            (-0.25_f64).to_bits()
        );
        assert_eq!(values[1].modified().unwrap().to_bits(), 0.25_f64.to_bits());
        let values = history
            .stored_versions(1)
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(values[0].value(), Some(""));
        assert_eq!(values[0].modified().unwrap().to_bits(), 1);
        assert_eq!(values[1].value(), None);
        assert_eq!(values[1].modified(), None);
        assert_eq!(history.stored_versions(2).unwrap().len(), 0);
        assert!(history.stored_versions(3).is_none());
    }
}

#[test]
fn history_mapping_requires_a_qualified_visible_memo_template_and_hidden_key() {
    let original = database();
    assert_eq!(
        read(&original, "missing").err(),
        Some(InspectError::UnknownColumn)
    );
    for name in ["Id", "Score"] {
        assert_eq!(read(&original, name).err(), Some(InspectError::Unsupported));
    }
    for (at, value, expected) in [
        (
            10 * PAGE_BYTES + FIELD_START + FIELD_BYTES + 16,
            0,
            InspectError::Unsupported,
        ),
        (
            TEMPLATE_PAGE * PAGE_BYTES + 16,
            1,
            InspectError::Unsupported,
        ),
        (
            TEMPLATE_PAGE * PAGE_BYTES + FIELD_START + 16,
            0x10,
            InspectError::Unsupported,
        ),
        (
            TEMPLATE_PAGE * PAGE_BYTES + FIELD_START + FIELD_BYTES + 15,
            2,
            InspectError::Malformed,
        ),
    ] {
        let mut changed = original.clone();
        changed[at] = value;
        assert_eq!(read(&changed, "Notes").err(), Some(expected));
    }
    let mut bytes = original.clone();
    bytes[TEMPLATE_PAGE * PAGE_BYTES..(TEMPLATE_PAGE + 1) * PAGE_BYTES].copy_from_slice(
        &definition(&[("Modified", 8, 0, 8), ("Notes", 12, 0, 0)], 0, 18 << 8),
    );
    assert_eq!(
        read(&bytes, "Notes")
            .unwrap()
            .stored_versions(0)
            .unwrap()
            .len(),
        2
    );
    for code in [1, 3] {
        let mut bytes = original.clone();
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        assert_eq!(read(&bytes, "Notes").err(), Some(InspectError::Unsupported));
    }
}

#[test]
fn nullable_parent_keys_stay_empty_and_orphan_versions_fail_before_publication() {
    let mut bytes = database();
    let page = 12 * PAGE_BYTES;
    let at = page
        + usize::from(u16::from_le_bytes(
            bytes[page + 18..page + 20].try_into().unwrap(),
        ));
    bytes[at + 16] &= !2;
    let history = read(&bytes, "Notes").unwrap();
    assert_eq!(history.links().parents()[2].key(), None);
    assert_eq!(history.stored_versions(2).unwrap().len(), 0);
    let mut bytes = database();
    let page = DATA_PAGE * PAGE_BYTES;
    let at = page
        + usize::from(u16::from_le_bytes(
            bytes[page + 14..page + 16].try_into().unwrap(),
        ));
    put32(&mut bytes, at + 2, 999);
    assert!(matches!(
        read(&bytes, "Notes"),
        Err(InspectError::ComplexReference(_))
    ));
}

#[test]
fn history_reads_keep_source_bytes_and_observe_resource_and_cancellation_limits() {
    let original = database();
    let mut source = Cursor::new(original.clone());
    let count = Cell::new(0);
    read_version_history_column(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Notes",
        &mut budget(),
        &|| {
            count.set(count.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in [0, count.get() / 2, count.get() - 1] {
        let calls = Cell::new(0);
        assert!(matches!(
            read_version_history_column(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Notes",
                &mut budget(),
                &|| {
                    let at = calls.get();
                    calls.set(at + 1);
                    at >= stop
                }
            ),
            Err(InspectError::Cancelled)
        ));
    }
    for mut limited in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(matches!(
            read_version_history_column(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Notes",
                &mut limited,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert_eq!(source.into_inner(), original);
}

proptest! {
    #![proptest_config(ProptestConfig {cases:128,
        rng_seed:proptest::test_runner::RngSeed::Fixed(0x484953544f525956),..Default::default()})]
    #[test]
    fn stored_versions_match_independent_nullable_text_and_exact_bit_sequences(
        // Keep synthetic uncompressed bytes outside the reserved FF FE header;
        // arbitrary Unicode (including FEFF/NUL) remains in the value suffix.
        values in prop::collection::vec((
            prop::option::of(".{0,80}".prop_map(|suffix| format!("v:{suffix}"))),
            prop::option::of(prop_oneof![
                Just(0_u64), Just(1), Just(0x8000000000000000),
                Just((-0.25_f64).to_bits()), Just(1.0000000001_f64.to_bits())
            ])
        ), 0..10)
    ) {
        let records = values.iter().enumerate()
            .map(|(i, (value, bits))| (10, i as i32 + 1, value.as_deref(), *bits))
            .collect::<Vec<_>>();
        let bytes = database_with(&records);
        let history = read(&bytes, "Notes").unwrap();
        let actual = history.stored_versions(0).unwrap()
            .map(|value| value.map(|version| (
                version.value().map(str::to_owned),
                version.modified().map(|date| date.to_bits())
            )))
            .collect::<Result<Vec<_>, _>>().unwrap();
        prop_assert_eq!(actual, values);
    }
}

#[test]
fn mixed_history_pages_preserve_visible_memo_and_exact_stored_versions() {
    use tablecraft_ace::{
        StoredCollectionColumn, StoredCollectionItem, StoredCollectionPageValue, TablePageRequest,
        read_stored_collection_table_page,
    };
    for code in [2, 5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        let page = read_stored_collection_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(1, 2).unwrap(),
            false,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(page.row_count(), 2);
        assert!(matches!(
            page.cell(0, 2),
            Some(StoredCollectionPageValue::Scalar(_))
        ));
        let Some(StoredCollectionColumn::VersionHistory(column)) = page.collection_column(1) else {
            panic!("Expected visible-Memo binding")
        };
        assert_eq!(column.memo_column().name(), "Notes");
        let StoredCollectionPageValue::Collection(cell) = page.cell(0, 1).unwrap() else {
            panic!("Expected stored versions")
        };
        assert_eq!(cell.key(), Some(20));
        let versions = cell
            .items()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let StoredCollectionItem::Version(first) = &versions[0] else {
            panic!("Expected version")
        };
        assert_eq!(first.value(), Some(""));
        assert_eq!(first.modified().unwrap().to_bits(), 1);
        let StoredCollectionItem::Version(null) = &versions[1] else {
            panic!("Expected version")
        };
        assert_eq!(null.value(), None);
        assert_eq!(null.modified(), None);
        let empty = read_stored_collection_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(u32::MAX, 1).unwrap(),
            false,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(empty.row_count(), 0);
        assert!(empty.reached_end());
        bytes[10 * PAGE_BYTES + FIELD_START + FIELD_BYTES + 16] = 0;
        assert!(matches!(
            read_stored_collection_table_page(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                TablePageRequest::new(u32::MAX, 1).unwrap(),
                false,
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
}

#[test]
fn explicit_history_limits_keep_visible_binding_and_complete_eof_checks() {
    let records = [(10, 1, Some("first"), Some(0)), (20, 2, None, None)];
    let bytes = database_with(&records);
    for (cap, accepted) in [(2, false), (3, true)] {
        let result = tablecraft_ace::read_version_history_column_with_limits(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Notes",
            tablecraft_ace::TableSnapshotLimits::new(cap, 1).unwrap(),
            &mut budget(),
            &|| false,
        );
        if accepted {
            assert_eq!(result.unwrap().stored_versions(0).unwrap().len(), 1);
        } else {
            assert_eq!(result.err(), Some(InspectError::StructureLimit));
        }
    }
    assert_eq!(bytes, database_with(&records));
}

#[test]
fn explicit_history_pages_keep_visible_memo_and_stored_version_bindings() {
    let records = [(10, 1, Some("first"), Some(0)), (20, 2, None, None)];
    let bytes = database_with(&records);
    let read = |cap| {
        read_stored_collection_table_page_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            CollectionPageRequest::new(
                TablePageRequest::new(1, 1).unwrap(),
                TableSnapshotLimits::new(cap, 1).unwrap(),
            ),
            &mut budget(),
            &|| false,
        )
    };
    assert_eq!(read(2).err(), Some(InspectError::StructureLimit));
    let page = read(3).unwrap();
    assert_eq!(page.row_count(), 1);
    let Some(StoredCollectionColumn::VersionHistory(history)) = page.collection_column(1) else {
        panic!("Expected a bound stored history");
    };
    assert_eq!(history.memo_column().name(), "Notes");
    assert_eq!(history.links().parents().len(), 3);
    assert_eq!(bytes, database_with(&records));
}
