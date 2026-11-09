use super::{SourceProfile, TableDropError, source, verify};
use crate::{InspectError, NativeImage, NativeTableStorage, inspect_catalog};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::{CatalogScope, ObjectKind};
use tablecraft_testkit::fixtures::access::{
    canonical_catalog_names, canonical_creator_rename,
    canonical_ordinary_indexed::Case,
    canonical_table_drop::{self as raw, Pair},
};

const SCOPE: CatalogScope = CatalogScope::new(927);
const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 4;
const CATALOG_DEFINITION: usize = 2;
const LIVE_COUNT_OFFSET: usize = 16;
const SLOT_DIRECTORY_OFFSET: usize = 14;
const SLOT_BYTES: usize = 2;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn assert_pair(pair: Pair, profile: SourceProfile) {
    let image = capture(&pair.before);
    let inspected = image
        .inspect_table_drop_source(&pair.table, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(inspected.table(), pair.table);
    assert_eq!(inspected.declared_rows(), 0);
    assert_eq!(inspected.identity().native_id(), 7);
    assert_eq!(inspected.identity().parent_id(), 0x0f00_0001);
    assert_eq!(inspected.identity().definition_page(), 7);
    assert_eq!(inspected.identity().catalog_page(), 4);
    assert_eq!(
        u16::from(inspected.identity().catalog_slot()),
        pair.catalog_slot
    );

    let plan = image
        .plan_table_drop(&pair.table, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), pair.before);
    assert_eq!(plan.after_bytes(), pair.after);
    assert_eq!(plan.before_bytes().len(), plan.after_bytes().len());
    assert_eq!(plan.table(), pair.table);
    assert_eq!(plan.identity(), inspected.identity());
    assert_eq!(plan.declared_rows(), 0);
    assert_eq!(plan.source_profile(), profile);

    let fresh = inspect_catalog(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(fresh.catalog().objects().len(), 8);
    assert!(fresh.origins().iter().all(|origin| origin.is_system()));
    assert!(fresh.origins().iter().all(|origin| origin.native_id() != 7));

    for page in 0..pair.before.len() / PAGE_BYTES {
        if ![CATALOG_PAGE, CATALOG_DEFINITION].contains(&page) {
            let range = page * PAGE_BYTES..(page + 1) * PAGE_BYTES;
            assert_eq!(pair.before[range.clone()], pair.after[range]);
        }
    }
    assert!(matches!(
        capture(plan.after_bytes()).plan_table_drop(&pair.table, &mut budget(), &|| false),
        Err(TableDropError::MissingTable)
    ));
}

#[test]
fn empty_inline_sources_match_complete_independent_images_for_both_ace16_headers() {
    for header in [5, 6] {
        for name in ["Items", "雪🙂", "\u{feff}Items"] {
            assert_pair(raw::unindexed(header, name, false), SourceProfile::Inline);
        }
    }
}

#[test]
fn empty_creator_sources_keep_all_allocated_definitions_data_indexes_and_properties() {
    let mut empty_sources = 0;
    for original in canonical_creator_rename::originals() {
        let pair = raw::creator(&original);
        let at = 7 * PAGE_BYTES + LIVE_COUNT_OFFSET;
        let rows = u32::from_le_bytes(pair.before[at..at + 4].try_into().unwrap());
        if rows == 0 {
            assert_pair(pair, SourceProfile::CreatorExternal);
            empty_sources += 1;
        } else {
            assert!(matches!(
                capture(&pair.before).plan_table_drop("Items", &mut budget(), &|| false),
                Err(TableDropError::Source)
            ));
        }
    }
    assert!(
        empty_sources > 0,
        "fixture set must retain actual empty creator sources"
    );
}

#[test]
fn populated_table_deletion_is_not_inferred_from_a_catalog_tombstone() {
    for header in [5, 6] {
        let original = raw::unindexed(header, "Items", true);
        assert!(matches!(
            capture(&original.before).inspect_table_drop_source("Items", &mut budget(), &|| false,),
            Err(TableDropError::Source)
        ));
        assert!(matches!(
            capture(&original.before).plan_table_drop("Items", &mut budget(), &|| false),
            Err(TableDropError::Source)
        ));

        for case in [
            Case::CounterPrimary,
            Case::UniqueManual,
            Case::NullableOrdinary,
            Case::AliasHistory,
            Case::RetainedTree,
            Case::InactiveIndexTail,
            Case::UnknownIndexHeader,
        ] {
            let indexed = raw::indexed(header, case, "Items");
            assert!(matches!(
                capture(&indexed.before).plan_table_drop("Items", &mut budget(), &|| false),
                Err(TableDropError::Source)
            ));
        }
    }
}

#[test]
fn absent_or_system_targets_are_errors_and_ascii_lookup_keeps_actual_spelling() {
    let pair = raw::unindexed(5, "Items", false);
    for missing in ["Missing", "MSysObjects", "MSysACEs", ""] {
        assert!(matches!(
            capture(&pair.before).plan_table_drop(missing, &mut budget(), &|| false),
            Err(TableDropError::MissingTable)
        ));
    }
    let plan = capture(&pair.before)
        .plan_table_drop("ITEMS", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.table(), "Items");
    assert_eq!(plan.after_bytes(), pair.after);

    let overlong = "x".repeat(65);
    let invalid = canonical_catalog_names::overlong_name_source(5, &overlong);
    assert!(matches!(
        capture(&invalid).plan_table_drop(&overlong, &mut budget(), &|| false),
        Err(TableDropError::Source)
    ));
}

#[test]
fn frozen_source_profiles_never_retry_each_other_and_missing_off_never_implies_disabled() {
    let inline = raw::unindexed(5, "Items", false);
    assert!(
        capture(&inline.before)
            .plan_table_drop_for_profile(
                "Items",
                SourceProfile::CreatorExternal,
                &mut budget(),
                &|| false,
            )
            .is_err()
    );

    for original in canonical_creator_rename::originals() {
        assert!(
            capture(&original)
                .plan_table_drop("Items", &mut budget(), &|| false)
                .is_err()
        );
        let pair = raw::creator(&original);
        assert!(
            capture(&pair.before)
                .plan_table_drop_for_profile("Items", SourceProfile::Inline, &mut budget(), &|| {
                    false
                },)
                .is_err()
        );
    }
}

#[test]
fn complete_image_audit_rejects_wrong_tombstone_count_body_or_allocation_bytes() {
    let pair = raw::unindexed(5, "Items", false);
    let image = capture(&pair.before);
    let qualified = source::qualify(
        &image,
        "Items",
        SourceProfile::Inline,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let slot = CATALOG_PAGE * PAGE_BYTES
        + SLOT_DIRECTORY_OFFSET
        + usize::from(pair.catalog_slot) * SLOT_BYTES;
    for position in [
        slot + 1,
        CATALOG_DEFINITION * PAGE_BYTES + LIVE_COUNT_OFFSET,
        CATALOG_PAGE * PAGE_BYTES + 2,
        3 * PAGE_BYTES + 4,
        7 * PAGE_BYTES + 30,
    ] {
        let mut changed = pair.after.clone();
        changed[position] ^= 1;
        assert!(matches!(
            verify::candidate(&image, &changed, &qualified, &mut budget(), &|| false),
            Err(TableDropError::Preservation)
        ));
    }
}

#[test]
fn cancellation_and_exhausted_budgets_never_return_a_partial_plan() {
    let pair = raw::unindexed(5, "Items", false);
    assert!(matches!(
        capture(&pair.before).plan_table_drop("Items", &mut budget(), &|| true),
        Err(TableDropError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        capture(&pair.before)
            .plan_table_drop("Items", &mut ResourceBudget::new(0, 0, 0), &|| false,),
        Err(TableDropError::Inspect(_))
    ));
    let fresh = capture(&pair.before);
    assert!(
        fresh
            .catalog()
            .catalog()
            .objects()
            .iter()
            .zip(fresh.catalog().origins())
            .any(|(object, origin)| object.kind() == ObjectKind::Table
                && origin.table_storage() == Some(NativeTableStorage::Local)
                && !origin.is_system())
    );
}
