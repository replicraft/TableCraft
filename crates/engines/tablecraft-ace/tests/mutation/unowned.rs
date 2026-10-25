use super::{budget, capture, index_fixture};
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{EditError, InspectError, Int32Update, WriteBlocker, check_table, read_table};
#[cfg(feature = "ace16-write-preview")]
use tablecraft_ace::{FixedColumnUpdates, FixedScalarUpdate};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const TABLE_DEFINITION_PAGE: usize = 10;
const LOGICAL_INDEX_COUNT_OFFSET: usize = 47;
const PHYSICAL_INDEX_COUNT_OFFSET: usize = 51;
const PHYSICAL_HEADER_START: usize = 63;
const PHYSICAL_HEADER_BYTES: usize = 12;
const ORIGINAL_PHYSICAL_DEFINITION_OFFSET: usize = 143;
const PHYSICAL_DEFINITION_BYTES: usize = 52;
#[cfg(feature = "ace16-write-preview")]
const SIGNATURE_OFFSET: usize = 4;
#[cfg(feature = "ace16-write-preview")]
const FORMAT_OFFSET: usize = 20;
#[cfg(feature = "ace16-write-preview")]
const ACE_SIGNATURE: &[u8] = b"Standard ACE DB\0";

fn with_unowned_second_definition() -> Vec<u8> {
    let mut bytes = index_fixture::database(false, false, false);
    let start = TABLE_DEFINITION_PAGE * PAGE_BYTES;
    let definition = &mut bytes[start..start + PAGE_BYTES];
    // Deliberately malformed metadata: insert an extra physical-header slot,
    // then duplicate its physical definition without adding a logical owner.
    let columns = PHYSICAL_HEADER_START + PHYSICAL_HEADER_BYTES;
    definition.copy_within(
        columns..PAGE_BYTES - PHYSICAL_HEADER_BYTES,
        columns + PHYSICAL_HEADER_BYTES,
    );
    definition.copy_within(PHYSICAL_HEADER_START..columns, columns);
    definition[PHYSICAL_INDEX_COUNT_OFFSET..PHYSICAL_INDEX_COUNT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&2_u32.to_le_bytes());

    let physical = ORIGINAL_PHYSICAL_DEFINITION_OFFSET + PHYSICAL_HEADER_BYTES;
    let logical = physical + PHYSICAL_DEFINITION_BYTES;
    definition.copy_within(
        logical..PAGE_BYTES - PHYSICAL_DEFINITION_BYTES,
        logical + PHYSICAL_DEFINITION_BYTES,
    );
    definition.copy_within(physical..logical, logical);
    bytes
}

fn without_logical_owner() -> Vec<u8> {
    let mut bytes = index_fixture::database(false, false, false);
    let offset = TABLE_DEFINITION_PAGE * PAGE_BYTES + LOGICAL_INDEX_COUNT_OFFSET;
    // Deliberate malformed-copy control: retain the physical Score index and
    // its original tree, but remove all logical declarations exposing it.
    bytes[offset..offset + size_of::<u32>()].copy_from_slice(&0_u32.to_le_bytes());
    bytes
}

#[test]
fn unowned_physical_index_cannot_hide_an_affected_key_from_jet4_planning() {
    let bytes = without_logical_owner();
    let check = check_table(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(check.rows(), 3);
    assert!(check.indexes().is_empty());

    assert_eq!(
        capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(1, 20, 27)],
                &mut budget(),
                &|| false,
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn)),
    );
}

#[test]
fn unowned_definitions_block_unrelated_changes_noops_and_empty_assignments() {
    for bytes in [without_logical_owner(), with_unowned_second_definition()] {
        for (column, expected) in [("Id", 2), ("Score", 20)] {
            for assignments in [
                vec![Int32Update::new(1, expected, expected + 7)],
                vec![Int32Update::new(1, expected, expected)],
                vec![],
            ] {
                assert_eq!(
                    capture(&bytes)
                        .plan_int32_updates("Items", column, &assignments, &mut budget(), &|| false)
                        .err(),
                    Some(EditError::Unsupported(WriteBlocker::IndexedColumn)),
                );
            }
        }
    }
}

#[test]
fn fully_owned_aliases_and_unindexed_tables_keep_their_existing_noop_permissions() {
    for bytes in [
        index_fixture::database(false, false, false),
        index_fixture::database(false, true, false),
        super::fixture::with_rule("Caption", "unchanged"),
    ] {
        for assignments in [vec![Int32Update::new(1, 20, 20)], vec![]] {
            let plan = capture(&bytes)
                .plan_int32_updates("Items", "Score", &assignments, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.changed_rows(), 0);
            assert_eq!(plan.before_bytes(), bytes);
            assert_eq!(plan.after_bytes(), bytes);
        }
        let plan = capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(1, 2, 9)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.changed_rows(), 1);
        let rows = read_table(
            &mut Cursor::new(plan.after_bytes()),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows()[1][0], Value::Int32(9));
    }
}

#[test]
fn ownership_proof_uses_caller_work_and_late_cancellation() {
    let bytes = with_unowned_second_definition();
    let calls = Cell::new(0);
    let mut allowance = budget();
    let initial_steps = allowance.remaining(Resource::Steps);
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates("Items", "Id", &[], &mut allowance, &|| {
                calls.set(calls.get() + 1);
                false
            })
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn)),
    );

    let final_check = calls.get();
    let consumed = initial_steps - allowance.remaining(Resource::Steps);
    let calls = Cell::new(0);
    assert_eq!(
        capture(&bytes)
            .plan_int32_updates("Items", "Id", &[], &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == final_check
            })
            .err(),
        Some(EditError::Inspect(InspectError::Cancelled)),
    );
    assert!(matches!(
        capture(&bytes).plan_int32_updates(
            "Items",
            "Id",
            &[],
            &mut ResourceBudget::new(64 * 1024 * 1024, 100_000, consumed - 1),
            &|| false,
        ),
        Err(EditError::Inspect(InspectError::Limit(_))),
    ));
}

#[cfg(feature = "ace16-write-preview")]
#[test]
fn ace_previews_refuse_unowned_physical_definitions_even_without_key_changes() {
    for code in [5, 6] {
        for mut bytes in [without_logical_owner(), with_unowned_second_definition()] {
            bytes[SIGNATURE_OFFSET..SIGNATURE_OFFSET + ACE_SIGNATURE.len()]
                .copy_from_slice(ACE_SIGNATURE);
            bytes[FORMAT_OFFSET] = code;
            for replacement in [None, Some(20), Some(27)] {
                let int32 = replacement
                    .into_iter()
                    .map(|value| Int32Update::new(1, 20, value))
                    .collect::<Vec<_>>();
                assert_eq!(
                    capture(&bytes)
                        .preview_ace16_int32_updates(
                            "Items",
                            "Score",
                            &int32,
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                    Some(EditError::Unsupported(WriteBlocker::IndexedColumn)),
                );
                let scalar = replacement
                    .into_iter()
                    .map(|value| FixedScalarUpdate::new(1, Value::Int32(20), Value::Int32(value)))
                    .collect::<Vec<_>>();
                assert_eq!(
                    capture(&bytes)
                        .preview_ace16_fixed_updates(
                            "Items",
                            "Score",
                            &scalar,
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                    Some(EditError::Unsupported(WriteBlocker::IndexedColumn)),
                );
                assert_eq!(
                    capture(&bytes)
                        .preview_ace16_batch_updates(
                            &[FixedColumnUpdates::new("Items", "Score", &scalar)],
                            &mut budget(),
                            &|| false,
                        )
                        .err(),
                    Some(EditError::Unsupported(WriteBlocker::IndexedColumn)),
                );
            }
        }
    }
}
