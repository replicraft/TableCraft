//! Independent current table DROP images and positive system-role refusal controls.
use super::*;
use crate::table::mutation::drop_table::row_effects as proof;
use crate::{PayloadPresence, inspect_catalog};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        self as fields, CASES, Case, readdition::Target,
    },
    canonical_creator_rename,
    canonical_table_drop::{
        self as old,
        current::{self as raw, HISTORIES, History},
    },
    database_properties,
};

const SCOPE: CatalogScope = CatalogScope::new(2902);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 100_000_000)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap()
}

fn word(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first difference {:?}",
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right)
    );
}

#[test]
fn current_table_drop_removes_live_roots_and_catalog_identity_without_reclaiming_history() {
    let mut exercised = 0;
    for case in CASES {
        for target in [Target::Number, Target::ShortText] {
            for filled in [false, true] {
                for history in HISTORIES {
                    // Initial sources have no readded target or first-fill state.
                    // Exercise each such original once rather than four times.
                    if matches!(history, History::Initial)
                        && (filled || matches!(target, Target::ShortText))
                    {
                        continue;
                    }
                    let original = raw::source(case, target, filled, history);
                    let expected = raw::after(&original);
                    let native = image(&original.bytes);
                    let inspection = native
                        .inspect_table_drop_source("Table1", &mut budget(), &|| false)
                        .unwrap_or_else(|error| {
                            panic!(
                                "inspect {case:?} {target:?} filled={filled} {history:?}: {error:?}"
                            )
                        });
                    assert_eq!(inspection.declared_rows() as usize, original.roots.len());
                    assert_eq!(inspection.identity().catalog_slot(), 7);
                    let plan = native
                        .plan_table_drop("Table1", &mut budget(), &|| false)
                        .unwrap_or_else(|error| {
                            panic!(
                                "plan {case:?} {target:?} filled={filled} {history:?}: {error:?}"
                            )
                        });
                    assert_eq!(plan.source_profile(), SourceProfile::Current);
                    assert_eq!(plan.declared_rows() as usize, original.roots.len());
                    equal_image(plan.before_bytes(), &original.bytes);
                    equal_image(plan.after_bytes(), &expected);
                    let catalog = inspect_catalog(
                        &mut Cursor::new(plan.after_bytes()),
                        SCOPE,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(catalog.origins().len(), 8);
                    assert!(catalog.origins().iter().all(|origin| origin.is_system()));
                    assert!(matches!(
                        image(plan.after_bytes()).inspect_table_drop_source(
                            "Table1",
                            &mut budget(),
                            &|| false
                        ),
                        Err(TableDropError::MissingTable)
                    ));
                    exercised += 1;
                }
            }
        }
    }
    assert_eq!(exercised, 119);
}

#[test]
fn current_table_drop_requires_actual_empty_permission_and_relationship_storage() {
    let original = raw::source(
        Case::MixedBinary,
        Target::ShortText,
        true,
        History::Inserted,
    );
    schema_dependencies::require_empty(&image(&original.bytes), &mut budget(), &|| false).unwrap();
    for definition in [5, 6] {
        let mut declared = original.bytes.to_vec();
        declared[definition * 4096 + 16..definition * 4096 + 20]
            .copy_from_slice(&1_u32.to_le_bytes());
        let native = image(&declared);
        assert!(schema_dependencies::require_empty(&native, &mut budget(), &|| false).is_err());
        assert!(
            native
                .inspect_table_drop_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
    }
    for slot in [1, 2, 5, 6] {
        let mut mapped = original.bytes.to_vec();
        let start = 3 * 4096 + word(&mapped, 3 * 4096 + 14 + slot * 2);
        mapped[start + 6] |= 1;
        let native = image(&mapped);
        assert!(schema_dependencies::require_empty(&native, &mut budget(), &|| false).is_err());
        assert!(
            native
                .plan_table_drop("Table1", &mut budget(), &|| false)
                .is_err()
        );
    }
    let mut renamed_schema = original.bytes.to_vec();
    let definition = 6 * 4096;
    let columns = word(&renamed_schema, definition + 45);
    let indexes = u32::from_le_bytes(
        renamed_schema[definition + 51..definition + 55]
            .try_into()
            .unwrap(),
    ) as usize;
    assert_eq!(columns, 4);
    assert_eq!(indexes, 0);
    let name = definition + 63 + indexes * 12 + columns * 25;
    assert_eq!(
        &renamed_schema[name..name + 8],
        &[6, 0, b'A', 0, b'C', 0, b'M', 0]
    );
    renamed_schema[name + 2..name + 8].copy_from_slice(&[b'B', 0, b'A', 0, b'D', 0]);
    let native = image(&renamed_schema);
    let parsed = inspect_table_schema(
        &mut native.reader(),
        SCOPE,
        "MSysACEs",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(parsed.columns()[0].name(), "BAD");
    current_schema::observe(&native, "Table1", &mut budget(), &|| false).unwrap();
    assert!(schema_dependencies::require_empty(&native, &mut budget(), &|| false).is_err());
    assert!(
        native
            .plan_table_drop("Table1", &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn current_table_drop_preserves_catalog_terminals_maps_payloads_and_counters() {
    let original = raw::source(
        Case::MixedBinary,
        Target::ShortText,
        true,
        History::Inserted,
    );
    let native = image(&original.bytes);
    let source = qualify(&native, "Table1", &mut budget(), &|| false).unwrap();
    let expected = raw::after(&original);
    proof::candidate(&native, &expected, &source, &mut budget(), &|| false).unwrap();
    let catalog_word = word(&original.bytes, 4 * 4096 + 14 + 2 * 7);
    assert_eq!(catalog_word & 0xc000, 0x4000);
    let reference = u32::from_le_bytes(
        original.bytes[4 * 4096 + (catalog_word & 0x1fff)..4 * 4096 + (catalog_word & 0x1fff) + 4]
            .try_into()
            .unwrap(),
    );
    let terminal_page = (reference >> 8) as usize * 4096;
    let terminal = terminal_page
        + (word(
            &original.bytes,
            terminal_page + 14 + 2 * (reference & 0xff) as usize,
        ) & 0x1fff);
    for offset in [
        3 * 4096 + 1000,
        5 * 4096 + 1000,
        7 * 4096 + 20,
        8 * 4096 + 4090,
        9 * 4096 + 482,
        10 * 4096 + 4090,
        terminal + 2,
    ] {
        let mut forged = expected.to_vec();
        forged[offset] ^= 1;
        assert!(matches!(
            proof::candidate(&native, &forged, &source, &mut budget(), &|| false),
            Err(TableDropError::Preservation)
        ));
    }
}

#[test]
fn current_table_drop_keeps_name_tracking_program_and_original_profile_boundaries() {
    let original = raw::source(
        Case::TextPresent,
        Target::ShortText,
        true,
        History::Inserted,
    );
    let disabled = database_properties::name_auto_correct_disabled();
    let start = original
        .bytes
        .windows(disabled.len())
        .position(|bytes| bytes == disabled)
        .unwrap();
    for entries in [
        [(1, 4, 1_i32.to_le_bytes()), (1, 4, 0_i32.to_le_bytes())],
        [(0, 4, 0_i32.to_le_bytes()), (1, 4, 0_i32.to_le_bytes())],
    ] {
        let borrowed: Vec<_> = entries
            .iter()
            .map(|(flags, kind, bytes)| (*flags, *kind, bytes.as_slice()))
            .collect();
        let properties = database_properties::name_auto_correct_values(&borrowed);
        assert_eq!(properties.len(), disabled.len());
        let mut blocked = original.bytes.to_vec();
        blocked[start..start + disabled.len()].copy_from_slice(&properties);
        assert!(
            image(&blocked)
                .inspect_table_drop_source("Table1", &mut budget(), &|| false)
                .is_err()
        );
    }
    let extra = fields::with_table_extra(Case::TextPresent);
    let native = image(&extra);
    let schema = inspect_table_schema(
        &mut native.reader(),
        SCOPE,
        "Table1",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        native
            .inspect_object_payloads(schema.object_id(), &mut budget(), &|| false)
            .unwrap()
            .extra(),
        PayloadPresence::Present
    );
    assert!(
        native
            .plan_table_drop("Table1", &mut budget(), &|| false)
            .is_err()
    );
    assert!(
        image(&original.bytes)
            .inspect_table_drop_source("Table1", &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        image(&original.bytes)
            .plan_table_drop("Table1", &mut ResourceBudget::new(0, 0, 0), &|| false)
            .is_err()
    );

    let populated = raw::source(Case::NumberAbsent, Target::Number, false, History::Initial);
    for profile in [SourceProfile::Inline, SourceProfile::CreatorExternal] {
        assert!(
            image(&populated.bytes)
                .plan_table_drop_for_profile("Table1", profile, &mut budget(), &|| false)
                .is_err()
        );
    }
    let empty = old::creator(&canonical_creator_rename::originals().remove(0));
    assert!(
        image(&empty.before)
            .plan_table_drop_for_profile(
                &empty.table,
                SourceProfile::Current,
                &mut budget(),
                &|| false
            )
            .is_err()
    );
    let legacy = image(&empty.before)
        .plan_table_drop(&empty.table, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(legacy.source_profile(), SourceProfile::CreatorExternal);
    equal_image(legacy.after_bytes(), &empty.after);
}
