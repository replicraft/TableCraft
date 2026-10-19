#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
mod support;

use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{InspectError, inspect_foreign_index_links, inspect_table_indexes};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ReferentialAction;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const LOGICAL_START: usize = DEFINITION_PAGE * PAGE_BYTES + 195;
const LOGICAL_BYTES: usize = 28;
const ROLE: usize = 12;
const OTHER_INDEX: usize = 13;
const OTHER_TABLE: usize = 17;
const UPDATE: usize = 21;
const DELETE: usize = 22;
const KIND: usize = 23;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}
fn database(code: u8, actions: u8) -> Vec<u8> {
    let mut bytes = index_fixture::database(false, true, false);
    bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    bytes[20] = code;
    for index in 0..2 {
        let start = LOGICAL_START + index * LOGICAL_BYTES;
        bytes[start + ROLE] = index as u8 + 1;
        support::put32(&mut bytes, start + OTHER_INDEX, 1 - index as u32);
        support::put32(&mut bytes, start + OTHER_TABLE, DEFINITION_PAGE as u32);
        bytes[start + UPDATE] = u8::from(actions != 0);
        bytes[start + DELETE] = actions;
        bytes[start + KIND] = 2;
    }
    relationship_rows(
        &mut bytes,
        &[relationship_row(
            "SelfScore",
            1 | if actions == 0 {
                0
            } else if actions == 1 {
                0x1100
            } else {
                0x2100
            },
            "Score",
            "Score",
        )],
    );
    bytes
}

fn relationship_row(name: &str, flags: u32, parent: &str, child: &str) -> Vec<u8> {
    relationship_column(name, flags, parent, child, 1, 0)
}
fn relationship_column(
    name: &str,
    flags: u32,
    parent: &str,
    child: &str,
    count: u32,
    ordinal: u32,
) -> Vec<u8> {
    let mut row = 8_u16.to_le_bytes().to_vec();
    for value in [flags, count, ordinal] {
        row.extend(value.to_le_bytes());
    }
    let mut offsets = vec![row.len() as u16];
    for value in [name, "Items", "Items", parent, child] {
        row.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
        offsets.push(row.len() as u16);
    }
    for offset in offsets.into_iter().rev() {
        row.extend(offset.to_le_bytes());
    }
    row.extend(5_u16.to_le_bytes());
    row.push(0xff);
    row
}

#[test]
fn composite_declaration_ordinals_can_reorder_but_cannot_cross_key_pairs() {
    let mut bytes = database(6, 0);
    let physical = DEFINITION_PAGE * PAGE_BYTES + 143;
    support::put16(&mut bytes, physical + 4, 0);
    support::put16(&mut bytes, physical + 7, 1);
    bytes[physical + 6] = 1;
    bytes[physical + 9] = 1;
    relationship_rows(
        &mut bytes,
        &[
            relationship_column("SelfPair", 1, "Id", "Id", 2, 1),
            relationship_column("SelfPair", 1, "Score", "Score", 2, 0),
        ],
    );
    assert_eq!(inspect(&bytes).unwrap().links().len(), 2);
    relationship_rows(
        &mut bytes,
        &[
            relationship_column("SelfPair", 1, "Id", "Score", 2, 1),
            relationship_column("SelfPair", 1, "Score", "Id", 2, 0),
        ],
    );
    assert_eq!(inspect(&bytes).err(), Some(InspectError::Malformed));
}
fn relationship_rows(bytes: &mut [u8], rows: &[Vec<u8>]) {
    support::put32(bytes, 7 * PAGE_BYTES + 16, rows.len() as u32);
    bytes[8 * PAGE_BYTES..9 * PAGE_BYTES].copy_from_slice(&support::data_page(
        1,
        &[(
            0,
            vec![0, 0, 0, 0, 0, 0, if rows.is_empty() { 0 } else { 2 }],
        )],
    ));
    bytes[9 * PAGE_BYTES..10 * PAGE_BYTES].copy_from_slice(&support::data_page(
        7,
        &rows.iter().cloned().map(|row| (0, row)).collect::<Vec<_>>(),
    ));
}
fn inspect(bytes: &[u8]) -> Result<tablecraft_ace::ForeignIndexInspection, InspectError> {
    inspect_foreign_index_links(
        &mut Cursor::new(bytes),
        CatalogScope::new(11),
        "Items",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn reciprocal_self_links_keep_logical_aliases_and_action_declarations() {
    for code in [5, 6] {
        for (actions, deletion) in [
            (0, ReferentialAction::Restrict),
            (1, ReferentialAction::Cascade),
            (2, ReferentialAction::SetNull),
        ] {
            let bytes = database(code, actions);
            let result = inspect(&bytes).unwrap();
            assert_eq!(result.links().len(), 2);
            for (position, link) in result.links().iter().enumerate() {
                let index = &result.indexes()[position];
                let reference = index.foreign_reference().unwrap();
                assert_eq!(reference.native_role(), position as u8 + 1);
                assert_eq!(reference.table_page(), DEFINITION_PAGE as u32);
                assert_eq!(reference.index_number(), 1 - position as u32);
                assert_eq!(reference.on_delete(), Some(deletion));
                assert_eq!(
                    reference.on_update(),
                    Some(if actions == 0 {
                        ReferentialAction::Restrict
                    } else {
                        ReferentialAction::Cascade
                    })
                );
                assert_eq!(link.index_number(), position as u32);
                assert_eq!(link.referenced_table(), result.schema().object_id());
                assert_eq!(link.referenced_index().number(), 1 - position as u32);
                assert_eq!(link.referenced_index().data_number(), index.data_number());
                // Reciprocal metadata alone cannot qualify physical foreign-index reads/writes.
                assert!(index.constraints().is_none());
            }
        }
    }
}

#[test]
fn broken_targets_and_reciprocals_return_no_partial_links() {
    for (offset, value, error) in [
        (OTHER_TABLE, 0, InspectError::Malformed),
        (OTHER_TABLE, 15, InspectError::Malformed),
        (OTHER_TABLE, 9, InspectError::Malformed),
        (OTHER_INDEX, 2, InspectError::Malformed),
        (OTHER_INDEX, u32::MAX, InspectError::Unsupported),
        (LOGICAL_BYTES + OTHER_INDEX, 1, InspectError::Malformed),
        (LOGICAL_BYTES + OTHER_TABLE, 9, InspectError::Malformed),
    ] {
        let mut bytes = database(6, 0);
        support::put32(&mut bytes, LOGICAL_START + offset, value);
        assert_eq!(inspect(&bytes).err(), Some(error));
    }
    for (offset, value, error) in [
        (ROLE, 0, InspectError::Unsupported),
        (ROLE, 3, InspectError::Unsupported),
        (UPDATE, 2, InspectError::Unsupported),
        (DELETE, 3, InspectError::Unsupported),
        (DELETE, 4, InspectError::Unsupported),
        (LOGICAL_BYTES + ROLE, 1, InspectError::Malformed),
        (LOGICAL_BYTES + KIND, 0, InspectError::Malformed),
        (LOGICAL_BYTES + UPDATE, 1, InspectError::Malformed),
        (LOGICAL_BYTES + DELETE, 1, InspectError::Malformed),
    ] {
        let mut bytes = database(6, 0);
        bytes[LOGICAL_START + offset] = value;
        let raw = inspect_table_indexes(
            &mut Cursor::new(&bytes),
            CatalogScope::new(11),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(raw.indexes().len(), 2);
        assert_eq!(inspect(&bytes).err(), Some(error));
    }
}

#[test]
fn link_inspection_never_traverses_user_rows_or_physical_index_pages() {
    let mut bytes = database(5, 0);
    bytes[12 * PAGE_BYTES..14 * PAGE_BYTES].fill(0x95);
    assert_eq!(inspect(&bytes).unwrap().links().len(), 2);
    let mut ordinary = index_fixture::node_database();
    ordinary[20] = 1;
    assert!(inspect(&ordinary).unwrap().links().is_empty());
}

#[test]
fn distinct_catalog_ids_aliasing_the_target_page_are_rejected() {
    let mut bytes = database(6, 0);
    let alias = support::record(0x0100_000a, 0, 1, 0, b"\xff\xfeAlias");
    let objects = [
        support::record(2, 0, 1, 0x8000_0002, b"\xff\xfeMSysObjects"),
        support::record(7, 0, 1, 0x8000_0002, b"\xff\xfeMSysRelationships"),
        alias,
        support::record(10, 0, 1, 0, b"\xff\xfeItems"),
    ];
    support::put32(&mut bytes, 2 * PAGE_BYTES + 16, objects.len() as u32);
    bytes[4 * PAGE_BYTES..5 * PAGE_BYTES].copy_from_slice(&support::data_page(
        2,
        &objects.into_iter().map(|row| (0, row)).collect::<Vec<_>>(),
    ));
    assert_eq!(inspect(&bytes).err(), Some(InspectError::Malformed));
}

#[test]
fn relationship_catalog_must_agree_with_each_link_and_cover_every_enforced_endpoint() {
    for (rows, error) in [
        (vec![], InspectError::Malformed),
        (
            vec![relationship_row("SelfScore", 0x101, "Score", "Score")],
            InspectError::Malformed,
        ),
        (
            vec![relationship_row("SelfScore", 2, "Score", "Score")],
            InspectError::Malformed,
        ),
        (
            vec![relationship_row("SelfScore", 4, "Score", "Score")],
            InspectError::Unsupported,
        ),
        (
            vec![relationship_row("SelfScore", 0x8000_0000, "Score", "Score")],
            InspectError::Unsupported,
        ),
        (
            vec![relationship_row("SelfScore", 1, "Id", "Score")],
            InspectError::Malformed,
        ),
        (
            vec![relationship_row("SelfScore", 1, "Score", "Missing")],
            InspectError::Malformed,
        ),
        (
            vec![
                relationship_row("SelfScore", 1, "Score", "Score"),
                relationship_row("Duplicate", 1, "Score", "Score"),
            ],
            InspectError::Malformed,
        ),
    ] {
        let mut bytes = database(6, 0);
        relationship_rows(&mut bytes, &rows);
        assert_eq!(inspect(&bytes).err(), Some(error));
    }
    let mut bytes = database(6, 0);
    for index in 0..2 {
        bytes[LOGICAL_START + index * LOGICAL_BYTES + KIND] = 0;
    }
    assert_eq!(inspect(&bytes).err(), Some(InspectError::Malformed));
    // Non-enforced catalog declarations require no foreign-index links.
    relationship_rows(
        &mut bytes,
        &[relationship_row("SelfScore", 2, "Score", "Score")],
    );
    assert!(inspect(&bytes).unwrap().links().is_empty());
    // A declared relationship cannot supply the missing parent uniqueness flag.
    let mut bytes = database(6, 0);
    bytes[DEFINITION_PAGE * PAGE_BYTES + 189] = 0x80;
    assert_eq!(inspect(&bytes).err(), Some(InspectError::Malformed));
}

#[test]
fn limits_and_early_and_late_cancellation_return_no_partial_report() {
    let bytes = database(6, 0);
    for mut budget in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(matches!(
            inspect_foreign_index_links(
                &mut Cursor::new(&bytes),
                CatalogScope::new(11),
                "Items",
                &mut budget,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    inspect_foreign_index_links(
        &mut Cursor::new(&bytes),
        CatalogScope::new(11),
        "Items",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let total = calls.get();
    assert!(total > 2);
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert_eq!(
            inspect_foreign_index_links(
                &mut Cursor::new(&bytes),
                CatalogScope::new(11),
                "Items",
                &mut budget(),
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() == stop
                }
            )
            .err(),
            Some(InspectError::Cancelled)
        );
    }
}

#[test]
fn registered_relationship_links_match_reciprocal_native_metadata() {
    let bytes = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../docs/examples/Access_Example_VBA.accdb"),
    )
    .unwrap();
    for table in ["TAG_GRP_TB", "TAG_NME_TB"] {
        let result = inspect_foreign_index_links(
            &mut Cursor::new(&bytes),
            CatalogScope::new(11),
            table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(result.links().len(), 1);
    }
}
