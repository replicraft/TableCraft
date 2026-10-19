use super::{
    DEFINITION_PAGE, LOGICAL_BYTES, LOGICAL_START, PAGE_BYTES, UPDATE, budget, database,
    index_fixture, inspect, support,
};
use std::{cell::Cell, io::Cursor, ops::Bound};
use tablecraft_ace::{
    IndexCheckStatus, IndexConstraintViolation, InspectError, Int32IndexSelection, check_table,
    read_table_index, read_table_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn read(bytes: &[u8], index: &str) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(11),
        "Items",
        index,
        &mut budget(),
        &|| false,
    )
}
fn check(bytes: &[u8]) -> Result<tablecraft_ace::TableCheck, InspectError> {
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(11),
        "Items",
        &mut budget(),
        &|| false,
    )
}

#[test]
fn qualified_foreign_aliases_read_leaf_node_and_nullable_trees_without_changing_raw_metadata() {
    for code in [5, 6] {
        for layout in 0..4 {
            let mut bytes = database(code, 0);
            if layout == 1 {
                let nodes = index_fixture::node_database();
                bytes.resize(nodes.len(), 0);
                bytes[13 * PAGE_BYTES..].copy_from_slice(&nodes[13 * PAGE_BYTES..]);
            } else if layout == 2 {
                let nullable = index_fixture::database(false, true, true);
                bytes[12 * PAGE_BYTES..14 * PAGE_BYTES]
                    .copy_from_slice(&nullable[12 * PAGE_BYTES..14 * PAGE_BYTES]);
            } else if layout == 3 {
                let descending = index_fixture::database(true, true, false);
                bytes[DEFINITION_PAGE * PAGE_BYTES + 149] = 0;
                bytes[13 * PAGE_BYTES..14 * PAGE_BYTES]
                    .copy_from_slice(&descending[13 * PAGE_BYTES..14 * PAGE_BYTES]);
            }
            let original = bytes.clone();
            for name in ["ScoreKey", "ScoreAlias"] {
                let rows = read(&bytes, name).unwrap();
                let mut expected = vec![
                    vec![
                        Value::Int32(1),
                        if layout == 2 {
                            Value::Null
                        } else {
                            Value::Int32(10)
                        },
                    ],
                    vec![Value::Int32(2), Value::Int32(20)],
                    vec![Value::Int32(3), Value::Int32(30)],
                ];
                if layout == 3 {
                    expected.reverse();
                }
                assert_eq!(rows.rows(), &expected);
            }
            let checked = check(&bytes).unwrap();
            assert_eq!(checked.rows(), 3);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|i| i.status() == IndexCheckStatus::Checked
                        && i.index().constraints().unwrap().unique())
            );
            // A completed check does not cache qualification into fresh raw inspection.
            assert!(
                inspect(&bytes)
                    .unwrap()
                    .indexes()
                    .iter()
                    .all(|i| i.constraints().is_none())
            );
            assert_eq!(bytes, original);
        }
    }
}

#[test]
fn foreign_ranges_keep_exact_numeric_bounds_and_null_selection() {
    let bytes = database(6, 0);
    let selected = read_table_index_range(
        &mut Cursor::new(&bytes),
        CatalogScope::new(11),
        "Items",
        "ScoreAlias",
        Int32IndexSelection::Range {
            lower: Bound::Included(20),
            upper: Bound::Excluded(30),
        },
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(selected.rows(), &[vec![Value::Int32(2), Value::Int32(20)]]);
    let mut bytes = bytes;
    let nullable = index_fixture::database(false, true, true);
    bytes[12 * PAGE_BYTES..14 * PAGE_BYTES]
        .copy_from_slice(&nullable[12 * PAGE_BYTES..14 * PAGE_BYTES]);
    let selected = read_table_index_range(
        &mut Cursor::new(&bytes),
        CatalogScope::new(11),
        "Items",
        "ScoreKey",
        Int32IndexSelection::Null,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(selected.rows(), &[vec![Value::Int32(1), Value::Null]]);
}

#[test]
fn unsupported_foreign_metadata_is_explicit_but_broken_links_fail_the_whole_check() {
    let mut bytes = database(6, 0);
    bytes[LOGICAL_START + 12] = 3;
    assert_eq!(
        read(&bytes, "ScoreKey").err(),
        Some(InspectError::Unsupported)
    );
    let report = check(&bytes).unwrap();
    assert!(
        report
            .indexes()
            .iter()
            .all(|i| i.status() == IndexCheckStatus::Unsupported
                && i.index().constraints().is_none())
    );
    let mut bytes = database(6, 0);
    for position in 0..2 {
        bytes[LOGICAL_START + position * LOGICAL_BYTES + UPDATE] = 1;
    }
    assert_eq!(
        read(&bytes, "ScoreKey").err(),
        Some(InspectError::Malformed)
    );
    assert_eq!(check(&bytes).err(), Some(InspectError::Malformed));
}

#[test]
fn physical_corruption_and_declared_foreign_uniqueness_are_checked_after_metadata() {
    let mut bytes = database(6, 0);
    bytes[13 * PAGE_BYTES] = 0;
    assert!(inspect(&bytes).is_ok());
    assert_eq!(
        read(&bytes, "ScoreKey").err(),
        Some(InspectError::Malformed)
    );
    assert_eq!(check(&bytes).err(), Some(InspectError::Malformed));
    let mut bytes = database(6, 0);
    let slot = usize::from(u16::from_le_bytes(
        bytes[12 * PAGE_BYTES + 16..12 * PAGE_BYTES + 18]
            .try_into()
            .unwrap(),
    )) & 0x1fff;
    support::put32(&mut bytes, 12 * PAGE_BYTES + slot + 6, 30);
    bytes[13 * PAGE_BYTES + 489] = 30;
    assert!(read(&bytes, "ScoreKey").is_ok());
    assert_eq!(
        check(&bytes).err(),
        Some(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        ))
    );
}

#[test]
fn foreign_reads_charge_shared_budgets_and_reject_early_or_late_cancellation() {
    let bytes = database(6, 0);
    for mut budget in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(matches!(
            read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(11),
                "Items",
                "ScoreKey",
                &mut budget,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    let calls = Cell::new(0);
    read_table_index(
        &mut Cursor::new(&bytes),
        CatalogScope::new(11),
        "Items",
        "ScoreKey",
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let total = calls.get();
    for stop in [1, total / 2, total] {
        calls.set(0);
        assert_eq!(
            read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(11),
                "Items",
                "ScoreKey",
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

#[cfg(feature = "ace16-write-preview")]
#[test]
fn checked_foreign_trees_without_parent_constraint_coverage_remain_blocked() {
    use tablecraft_ace::{EditError, FixedScalarUpdate, Int32Update, NativeImage, WriteBlocker};
    let bytes = database(6, 0);
    assert!(
        check(&bytes)
            .unwrap()
            .indexes()
            .iter()
            .all(|i| i.status() == IndexCheckStatus::Checked)
    );
    let image = NativeImage::capture(
        &mut Cursor::new(&bytes),
        CatalogScope::new(11),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        image
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(
                    0,
                    Value::Int32(10),
                    Value::Int32(11)
                )],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
    // The original raw physical declaration still cannot act as a write capability.
    assert_eq!(bytes[DEFINITION_PAGE * PAGE_BYTES + 189], 0x81);
    let mut jet4 = bytes;
    jet4[4..20].copy_from_slice(b"Standard Jet DB\0");
    jet4[20] = 1;
    let image = NativeImage::capture(
        &mut Cursor::new(&jet4),
        CatalogScope::new(11),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        image
            .plan_int32_updates(
                "Items",
                "Score",
                &[Int32Update::new(0, 10, 11)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::ConstraintCoverage))
    );
}
