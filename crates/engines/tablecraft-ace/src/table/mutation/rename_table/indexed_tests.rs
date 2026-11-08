//! Independent whole-image oracles for the explicit unpublished indexed profile.
use crate::{IndexCheckStatus, NativeImage, TableRenameError, inspect_table_indexes, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_indexed_rename::pair, canonical_ordinary_indexed::Case, database_properties,
};

const SCOPE: CatalogScope = CatalogScope::new(741);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn indexed_candidates_match_complete_independent_images_and_fresh_membership() {
    for header in [5, 6] {
        for case in [
            Case::CounterPrimary,
            Case::UniqueManual,
            Case::NullableOrdinary,
            Case::AliasHistory,
            Case::RetainedTree,
            Case::InactiveIndexTail,
            // INDEX write stamps are outside the catalog edit. The retained
            // reader accepts them; an index writer's zero-stamp rule does not
            // authorize changing or dropping these original opaque bytes.
            Case::UnknownIndexHeader,
        ] {
            for (old, new) in [
                ("Items", "Inventory"),
                ("Inventory", "雪🙂"),
                ("Items", "Items"),
            ] {
                let expected = pair(header, case, old, new);
                let plan = capture(&expected.before)
                    .plan_indexed_table_rename(old, new, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(plan.before_bytes(), expected.before);
                assert_eq!(plan.after_bytes(), expected.after);
                assert_eq!(plan.changed(), old != new);
                assert_eq!(plan.identity().native_id(), 7);
                assert_eq!(plan.identity().catalog_page(), 4);
                assert_eq!(plan.identity().catalog_slot(), 8);
                let rows = read_table(
                    &mut Cursor::new(plan.after_bytes()),
                    SCOPE,
                    new,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(rows.rows(), expected.rows);
                let indexes = inspect_table_indexes(
                    &mut Cursor::new(plan.after_bytes()),
                    SCOPE,
                    new,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    indexes
                        .indexes()
                        .iter()
                        .map(|index| index.name())
                        .collect::<Vec<_>>(),
                    expected.indexes
                );
                let checked = crate::table::indexes::check_table_retained(
                    &mut Cursor::new(plan.after_bytes()),
                    SCOPE,
                    new,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(
                    checked
                        .indexes()
                        .iter()
                        .all(|index| index.status() == IndexCheckStatus::Checked)
                );
                for page in expected.index_pages {
                    let bytes = page as usize * 4096;
                    assert_eq!(
                        &plan.before_bytes()[bytes..bytes + 4096],
                        &plan.after_bytes()[bytes..bytes + 4096]
                    );
                }
            }
        }
    }
}

#[test]
fn original_planner_and_frozen_recoverable_profile_still_refuse_indexes() {
    let source = pair(5, Case::CounterPrimary, "Items", "Renamed");
    assert!(matches!(
        capture(&source.before).plan_table_rename("Items", "Renamed", &mut budget(), &|| false),
        Err(TableRenameError::Source)
    ));
    assert!(
        capture(&source.before)
            .plan_recoverable_table_rename("Items", "Renamed", &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn malformed_index_keys_membership_aliases_and_metadata_refuse_even_for_noops() {
    for case in [
        Case::OrphanPhysical,
        Case::IncompatibleAlias,
        Case::IndexDataOverlap,
        Case::FreeIndex,
        Case::MissingOwned,
        Case::UnreachableOwned,
        Case::WrongKey,
        Case::WrongRow,
        Case::BadNodeBound,
    ] {
        let source = pair(5, case, "Items", "Items");
        assert!(
            capture(&source.before)
                .plan_indexed_table_rename("Items", "Items", &mut budget(), &|| false)
                .is_err(),
            "{}",
            case.id()
        );
    }
}

#[test]
fn explicit_indexed_profile_keeps_disabled_tracking_and_cancellation_guards() {
    let source = pair(5, Case::CounterPrimary, "Items", "Renamed");
    let mut unknown = source.before.clone();
    let properties = database_properties::name_auto_correct_disabled();
    let start = unknown
        .windows(properties.len())
        .position(|bytes| bytes == properties)
        .unwrap();
    let last_value = start + properties.len() - 4;
    unknown[last_value..last_value + 4].copy_from_slice(&1_i32.to_le_bytes());
    assert!(matches!(
        capture(&unknown).plan_indexed_table_rename("Items", "Renamed", &mut budget(), &|| false),
        Err(TableRenameError::Dependency)
    ));
    assert!(
        capture(&source.before)
            .plan_indexed_table_rename("Items", "Renamed", &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        capture(&source.before)
            .plan_indexed_table_rename(
                "Items",
                "Renamed",
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
}
