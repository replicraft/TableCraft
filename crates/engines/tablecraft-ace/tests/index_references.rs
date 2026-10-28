#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
use fixture as base;
#[allow(dead_code)]
#[path = "attachments/fixture.rs"]
mod attachment_fixture;
#[allow(dead_code)]
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
#[allow(dead_code)]
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
use std::{cell::Cell, io::Cursor, ops::Bound};
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, InspectError, Int32IndexSelection,
    NativeRowReference, TableSnapshotLimits, inspect_table_schema, read_attachment_column,
    read_attachment_metadata, read_table_index_with_references, read_table_index_with_request,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE: usize = 4096;
const BYTES: u64 = 64 * 1024 * 1024;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, 100_000, 1_000_000)
}

fn request(selection: IndexProjectionSelection<'_>) -> IndexReadRequest<'_> {
    IndexReadRequest {
        columns: Some(&[]),
        selection,
        maximum_rows: 3,
    }
}

fn positions(references: &[NativeRowReference]) -> Vec<(u32, u8)> {
    references
        .iter()
        .map(|reference| (reference.page_number(), reference.slot_number()))
        .collect()
}

#[test]
fn indexed_zero_width_rows_keep_exact_logical_order_nulls_aliases_and_subsets() {
    for code in [5, 6] {
        for descending in [false, true] {
            let mut bytes = index_fixture::database(descending, true, true);
            bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
            bytes[20] = code;
            let expected = if descending {
                vec![(12, 2), (12, 1), (12, 0)]
            } else {
                vec![(12, 0), (12, 1), (12, 2)]
            };
            for index in ["ScoreKey", "ScoreAlias"] {
                let indexed = read_table_index_with_references(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    index,
                    request(IndexProjectionSelection::All),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(indexed.snapshot().rows(), &[vec![], vec![], vec![]]);
                assert_eq!(positions(indexed.row_references()), expected);
            }
            let indexed = read_table_index_with_references(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "ScoreKey",
                request(IndexProjectionSelection::Int32(
                    Int32IndexSelection::Range {
                        lower: Bound::Included(20),
                        upper: Bound::Included(20),
                    },
                )),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let (snapshot, references) = indexed.into_parts();
            assert_eq!(snapshot.rows(), &[vec![]]);
            assert_eq!(positions(&references), [(12, 1)]);
        }
    }
}

fn row(id: i32, score: Option<i32>) -> Vec<u8> {
    let mut bytes = 2_u16.to_le_bytes().to_vec();
    bytes.extend(id.to_le_bytes());
    bytes.extend(score.unwrap_or(0).to_le_bytes());
    bytes.push(if score.is_some() { 3 } else { 1 });
    bytes
}

#[test]
fn indexed_forwarding_keeps_original_root_and_exact_schema_binding() {
    let mut bytes = index_fixture::database(false, false, false);
    bytes.resize(16 * PAGE, 0);
    bytes[11 * PAGE + PAGE - 1] = 0x90;
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(
        10,
        &[
            (0x4000, (15_u32 << 8).to_le_bytes().to_vec()),
            (0, row(2, Some(20))),
            (0, row(3, Some(30))),
        ],
    ));
    bytes[15 * PAGE..16 * PAGE]
        .copy_from_slice(&support::data_page(10, &[(0x8000, row(1, Some(10)))]));
    let indexed = read_table_index_with_references(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        IndexReadRequest {
            columns: Some(&["Id"]),
            ..request(IndexProjectionSelection::All)
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        positions(indexed.row_references()),
        [(12, 0), (12, 1), (12, 2)]
    );
    assert_eq!(indexed.snapshot().rows()[0], [Value::Int32(1)]);
    let schema = inspect_table_schema(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(schema.same_definition(indexed.snapshot().schema()));
    let foreign_scope = inspect_table_schema(
        &mut Cursor::new(&bytes),
        CatalogScope::new(2),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(!schema.same_definition(&foreign_scope));
    // Both changes leave declared rows and fixed_end unchanged. They must still
    // invalidate the captured native definition after fields leave Layout.fields.
    for column_byte in [15, 21] {
        let mut drift = bytes.clone();
        drift[10 * PAGE + 75 + column_byte] ^= 2;
        let drifted = inspect_table_schema(
            &mut Cursor::new(&drift),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(!schema.same_definition(&drifted));
    }
}

#[test]
fn complete_parent_references_include_absent_keys_and_skip_hidden_forward_targets() {
    let mut bytes = attachment_fixture::database();
    bytes.resize(27 * PAGE, 0);
    support::put32(&mut bytes, 10 * PAGE + 16, 4);
    bytes[11 * PAGE..12 * PAGE].copy_from_slice(&support::data_page(
        1,
        &[(0, vec![0, 0, 0, 0, 0, 0, 0x10, 0, 4])],
    ));
    bytes[12 * PAGE..13 * PAGE].copy_from_slice(&support::data_page(
        10,
        &[
            (0, row(99, Some(10))),
            (0, row(99, Some(20))),
            (0x4000, (26_u32 << 8).to_le_bytes().to_vec()),
            (0, row(99, None)),
        ],
    ));
    bytes[26 * PAGE..27 * PAGE]
        .copy_from_slice(&support::data_page(10, &[(0x8000, row(99, Some(30)))]));
    let full = read_attachment_column(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let metadata = read_attachment_metadata(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        positions(
            &full
                .links()
                .parents()
                .iter()
                .map(|parent| parent.row_reference())
                .collect::<Vec<_>>()
        ),
        [(12, 0), (12, 1), (12, 2), (12, 3)]
    );
    assert!(
        full.links()
            .schema()
            .owner()
            .same_definition(metadata.schema().owner())
    );
    for (full, metadata) in full.links().parents().iter().zip(metadata.parents()) {
        assert!(full.row_reference() == metadata.row_reference());
    }
    assert_eq!(
        full.links()
            .parents()
            .iter()
            .map(|parent| (parent.key(), parent.child_count()))
            .collect::<Vec<_>>(),
        [(Some(10), 2), (Some(20), 1), (Some(30), 0), (None, 0)]
    );
}

#[test]
fn reference_retention_spends_extra_budget_and_never_publishes_cancelled_results() {
    let bytes = index_fixture::database(false, false, false);
    let mut measured = budget();
    read_table_index_with_request(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        request(IndexProjectionSelection::All),
        &mut measured,
        &|| false,
    )
    .unwrap();
    let consumed = BYTES - measured.remaining(Resource::Bytes);
    read_table_index_with_request(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        request(IndexProjectionSelection::All),
        &mut ResourceBudget::new(consumed, 100_000, 1_000_000),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        read_table_index_with_references(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "ScoreKey",
            request(IndexProjectionSelection::All),
            &mut ResourceBudget::new(consumed, 100_000, 1_000_000),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    let calls = Cell::new(0);
    read_table_index_with_references(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        request(IndexProjectionSelection::All),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let terminal = calls.get() - 1;
    calls.set(0);
    assert!(matches!(
        read_table_index_with_references(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "ScoreKey",
            request(IndexProjectionSelection::All),
            &mut budget(),
            &|| {
                let count = calls.get();
                calls.set(count + 1);
                count == terminal
            }
        ),
        Err(InspectError::Cancelled)
    ));
    assert!(matches!(
        read_table_index_with_references(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "ScoreKey",
            IndexReadRequest {
                maximum_rows: TableSnapshotLimits::MAX_ROWS + 1,
                ..request(IndexProjectionSelection::All)
            },
            &mut budget(),
            &|| false
        ),
        Err(InspectError::InvalidSnapshotLimits)
    ));
}
