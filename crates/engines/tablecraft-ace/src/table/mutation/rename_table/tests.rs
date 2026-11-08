//! Complete independent image oracles, distinct from native Access comparison.
use super::*;
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_catalog_names::{RenamePair, rename_pair},
    canonical_creation, database_properties,
};

const SCOPE: CatalogScope = CatalogScope::new(41);
const BYTES: u64 = 512 * 1024 * 1024;
const STEPS: u64 = 50_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, 100_000, STEPS)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn catalog_row(bytes: &[u8], slot: usize) -> std::ops::Range<usize> {
    let page = 4 * 4096;
    let at = page + 14 + 2 * slot;
    let start = usize::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()));
    let end = if slot == 0 {
        4096
    } else {
        usize::from(u16::from_le_bytes(bytes[at - 2..at].try_into().unwrap()))
    };
    page + start..page + end
}

fn assert_pair(pair: RenamePair) {
    let plan = capture(&pair.before)
        .plan_table_rename(&pair.old_name, &pair.new_name, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.before_bytes(), pair.before);
    assert_eq!(plan.after_bytes(), pair.after);
    assert_eq!(plan.table(), pair.old_name);
    assert_eq!(plan.new_name(), pair.new_name);
    assert_eq!(plan.changed(), pair.old_name != pair.new_name);
    assert_eq!(plan.identity().native_id(), 7);
    assert_eq!(plan.identity().parent_id(), 0x0f00_0001);
    assert_eq!(plan.identity().definition_page(), 7);
    assert_eq!(plan.identity().catalog_page(), 4);
    assert_eq!(plan.identity().catalog_slot(), 8);
    let schema = crate::inspect_table_schema(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        &pair.new_name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(schema.object_id(), plan.identity().object_id());
    assert_eq!(schema.definition_page(), 7);
    let rows = crate::read_table(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        &pair.new_name,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows().len(), pair.rows.len());
    for (actual, expected) in rows.rows().iter().zip(pair.rows) {
        assert_eq!(
            actual,
            &expected.into_iter().map(Value::Int32).collect::<Vec<_>>()
        );
    }
}

#[test]
fn complete_independent_images_cover_both_headers_growth_shrink_unicode_and_spelling() {
    for header in [5, 6] {
        for populated in [false, true] {
            for (old, new) in [
                ("Items", "Inventory"),
                ("Inventory", "雪🙂"),
                ("Items", "items"),
                ("Items", "Items"),
                ("\u{feff}雪🙂", "\u{feff}āΩ雪🙂A"),
            ] {
                assert_pair(rename_pair(header, old, new, populated));
            }
        }
    }
    assert_pair(rename_pair(5, "Items", &"\u{feff}".repeat(64), true));
}

#[test]
fn original_spelling_timestamps_and_unowned_complement_remain_exact() {
    let mut pair = rename_pair(5, "Items", "Archive", true);
    for bytes in [&mut pair.before, &mut pair.after] {
        let row = catalog_row(bytes, 8);
        bytes[row.start + 20..row.start + 28].copy_from_slice(&45_000.5_f64.to_le_bytes());
        bytes.extend_from_slice(&[0x65; 4096]);
    }
    let plan = capture(&pair.before)
        .plan_table_rename("ITEMS", &pair.new_name, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.table(), "Items");
    assert_eq!(plan.after_bytes(), pair.after);
}

#[test]
fn native_identifiers_and_reserved_namespace_collisions_refuse() {
    let pair = rename_pair(5, "Items", "Other", false);
    for name in [
        "",
        " leading",
        "a.b",
        "a!b",
        "a`b",
        "a[b",
        "a]b",
        "line\nfeed",
        &"x".repeat(65),
        &"🙂".repeat(33),
    ] {
        let result =
            capture(&pair.before).plan_table_rename("Items", name, &mut budget(), &|| false);
        assert!(
            matches!(result, Err(TableRenameError::InvalidName)),
            "{name:?}"
        );
    }
    for name in ["MSysObjects", "msysobjects", "Tables", "DatabaseRoot"] {
        let result =
            capture(&pair.before).plan_table_rename("Items", name, &mut budget(), &|| false);
        assert!(
            matches!(result, Err(TableRenameError::DuplicateName)),
            "{name}"
        );
    }
}

#[test]
fn disabled_name_tracking_is_positive_evidence_and_noop_cannot_bypass_dependencies() {
    let original = canonical_creation::source(canonical_creation::Case::Empty).bytes;
    let result = capture(&original).plan_table_rename("Items", "Items", &mut budget(), &|| false);
    assert!(matches!(result, Err(TableRenameError::Dependency)));

    for (flags, kind, value) in [(1, 4, 1_i32), (0, 4, 0), (1, 3, 0)] {
        let mut pair = rename_pair(5, "Items", "Other", false);
        let bag = database_properties::name_auto_correct_values(&[
            (flags, kind, &value.to_le_bytes()),
            (1, 4, &0_i32.to_le_bytes()),
        ]);
        let row = database_properties::native17_database_record(&bag);
        let range = catalog_row(&pair.before, 7);
        assert_eq!(range.len(), row.len());
        pair.before[range].copy_from_slice(&row);
        assert!(
            capture(&pair.before)
                .plan_table_rename("Items", "Items", &mut budget(), &|| false)
                .is_err()
        );
    }
    let mut pair = rename_pair(5, "Items", "Other", false);
    let row = catalog_row(&pair.before, 7);
    pair.before[row.end - 2] &= !0x40;
    assert!(matches!(
        capture(&pair.before).plan_table_rename("Items", "Other", &mut budget(), &|| false),
        Err(TableRenameError::Dependency)
    ));

    let pair = rename_pair(5, "Items", "Other", false);
    let mut records = (0..9)
        .map(|slot| (0, pair.before[catalog_row(&pair.before, slot)].to_vec()))
        .collect::<Vec<_>>();
    let user = &mut records[8].1;
    // Independently author a present inline LvExtra, leaving every preceding
    // variable field absent. Native17 stores 12 offsets, a count and 3 mask bytes.
    let footer = user.len() - 12 * 2 - 2 - 3;
    let original_end = u16::from_le_bytes(user[footer..footer + 2].try_into().unwrap());
    let mut extra = 0x8000_0004_u32.to_le_bytes().to_vec();
    extra.extend_from_slice(&[0; 8]);
    extra.extend_from_slice(b"TCEX");
    user.splice(footer..footer, extra);
    let footer = footer + 16;
    user[footer..footer + 2].copy_from_slice(&(original_end + 16).to_le_bytes());
    *user.last_mut().unwrap() |= 1;
    let payload_bytes = records.iter().map(|(_, row)| row.len()).sum::<usize>();
    let mut page = tablecraft_testkit::fixtures::access::data_page(2, &records);
    page[2..4].copy_from_slice(
        &u16::try_from(4096 - 14 - 2 * 9 - payload_bytes)
            .unwrap()
            .to_le_bytes(),
    );
    let mut source = pair.before;
    source[4 * 4096..5 * 4096].copy_from_slice(&page);

    let image = capture(&source);
    let target = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == 7)
        .unwrap()
        .object_id();
    let payloads = image
        .read_object_payloads(target, &mut budget(), &|| false)
        .unwrap();
    assert_eq!(payloads.extra().presence(), crate::PayloadPresence::Present);
    assert_eq!(payloads.extra().as_bytes(), Some(b"TCEX".as_slice()));
    assert!(matches!(
        image.plan_table_rename("Items", "Items", &mut budget(), &|| false),
        Err(TableRenameError::Dependency)
    ));
}

#[test]
fn selected_profile_and_private_capture_purpose_remain_independent() {
    let pair = rename_pair(5, "Items", "Other", false);
    let fixed = NativeImage::capture_for_fixed_updates(
        &mut Cursor::new(&pair.before),
        SCOPE,
        crate::FixedUpdateImageLimits::default(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        fixed.plan_table_rename("Items", "Other", &mut budget(), &|| false),
        Err(TableRenameError::Inspect(InspectError::StructureLimit))
    ));
    let mut old_profile = pair.before;
    old_profile[20] = 3;
    assert!(matches!(
        capture(&old_profile).plan_table_rename("Items", "Other", &mut budget(), &|| false),
        Err(TableRenameError::Source)
    ));
}

#[test]
fn exact_cumulative_quotas_and_actual_final_cancellation_prevent_candidate_publication() {
    let pair = rename_pair(5, "Items", "Inventory", true);
    let mut measured = budget();
    let calls = Cell::new(0);
    capture(&pair.before)
        .plan_table_rename("Items", "Inventory", &mut measured, &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let used_bytes = BYTES - measured.remaining(Resource::Bytes);
    let used_steps = STEPS - measured.remaining(Resource::Steps);
    for limits in [
        ResourceBudget::new(used_bytes - 1, 100_000, STEPS),
        ResourceBudget::new(BYTES, 100_000, used_steps - 1),
    ] {
        let mut limits = limits;
        let result =
            capture(&pair.before).plan_table_rename("Items", "Inventory", &mut limits, &|| false);
        assert!(matches!(
            result,
            Err(TableRenameError::Inspect(InspectError::Limit(_)))
        ));
    }
    let final_check = calls.get();
    for boundary in [1, final_check / 2, final_check] {
        calls.set(0);
        let result =
            capture(&pair.before).plan_table_rename("Items", "Inventory", &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() == boundary
            });
        assert!(
            matches!(
                result,
                Err(TableRenameError::Inspect(InspectError::Cancelled))
            ),
            "boundary {boundary}"
        );
    }
}

#[test]
fn another_table_free_candidate_cannot_claim_the_renamed_catalog_page() {
    let mut pair = rename_pair(5, "Items", "Other", false);
    let reference = u32::from_le_bytes(
        pair.before[7 * 4096 + 59..7 * 4096 + 63]
            .try_into()
            .unwrap(),
    );
    let page = (reference >> 8) as usize * 4096;
    let slot = (reference & 255) as usize;
    let at = page + 14 + slot * 2;
    let start = usize::from(u16::from_le_bytes(
        pair.before[at..at + 2].try_into().unwrap(),
    ));
    pair.before[page + start + 5] |= 1 << 4;
    let result =
        capture(&pair.before).plan_table_rename("Items", "Other", &mut budget(), &|| false);
    assert!(matches!(result, Err(TableRenameError::Source)));
}

#[test]
fn unknown_definition_tail_and_flagged_native_id_do_not_gain_rename_authority() {
    let mut pair = rename_pair(5, "Items", "Other", false);
    let definition = 7 * 4096;
    let length = u32::from_le_bytes(
        pair.before[definition + 8..definition + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    pair.before[definition + length] = 0x65;
    pair.before[definition + 8..definition + 12]
        .copy_from_slice(&((length + 1) as u32).to_le_bytes());
    let free = u16::from_le_bytes(
        pair.before[definition + 2..definition + 4]
            .try_into()
            .unwrap(),
    );
    pair.before[definition + 2..definition + 4].copy_from_slice(&(free - 1).to_le_bytes());
    let result =
        capture(&pair.before).plan_table_rename("Items", "Other", &mut budget(), &|| false);
    assert!(matches!(result, Err(TableRenameError::Source)));

    let mut pair = rename_pair(5, "Items", "Other", false);
    let row = catalog_row(&pair.before, 8);
    pair.before[row.start + 2..row.start + 6].copy_from_slice(&0x0100_0007_u32.to_le_bytes());
    let result =
        capture(&pair.before).plan_table_rename("Items", "Other", &mut budget(), &|| false);
    assert!(matches!(result, Err(TableRenameError::Source)));
}

#[test]
fn actual_rename_source_exposes_both_exact_disabled_database_settings() {
    for header in [5, 6] {
        let pair = rename_pair(header, "Items", "Other", false);
        let properties = crate::inspect_database_properties(
            &mut Cursor::new(&pair.before),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(properties.groups().len(), 1);
        let group = &properties.groups()[0];
        assert_eq!(group.kind(), crate::PropertyGroupKind::Table);
        assert!(group.name().is_empty());
        assert_eq!(group.properties().len(), 2);
        for name in ["Track Name AutoCorrect Info", "Perform Name AutoCorrect"] {
            let property = group
                .properties()
                .iter()
                .find(|property| property.name() == name)
                .unwrap();
            assert_eq!(property.native_type(), 4);
            assert_eq!(property.flags(), 1);
            assert_eq!(property.raw_value(), &0_i32.to_le_bytes());
        }
    }
}

#[test]
fn flagged_system_table_id_cannot_alias_an_empty_user_definition() {
    let mut pair = rename_pair(5, "Items", "Other", false);
    let aces = catalog_row(&pair.before, 6);
    pair.before[aces.start + 2..aces.start + 6].copy_from_slice(&0x0100_0007_u32.to_le_bytes());
    let inventory = crate::inspect_catalog(
        &mut Cursor::new(&pair.before),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(inventory.catalog().objects().len(), 9);
    let result =
        capture(&pair.before).plan_table_rename("Items", "Items", &mut budget(), &|| false);
    assert!(matches!(result, Err(TableRenameError::Source)));
}

#[test]
fn readable_catalog_with_retired_variable_payload_cannot_bypass_noop_dependency_checks() {
    for header in [5, 6] {
        let bytes =
            tablecraft_testkit::fixtures::access::canonical_catalog_names::retired_variable_source(
                header,
            );
        let inventory =
            crate::inspect_catalog(&mut Cursor::new(&bytes), SCOPE, &mut budget(), &|| false)
                .unwrap();
        assert_eq!(inventory.catalog().objects().len(), 9);
        assert!(
            inventory
                .catalog()
                .objects()
                .iter()
                .any(|object| object.name().as_str() == "Items")
        );
        let schema = crate::inspect_table_schema(
            &mut Cursor::new(&bytes),
            SCOPE,
            "MSysObjects",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(schema.columns().len(), 17);
        assert_eq!(schema.layout.max_columns, 18);
        assert_eq!(schema.layout.variable_slots, 12);

        // Inert Id/LvProp selection remains valid, including the exact disabled
        // policy facts. Inspection permission does not imply mutation coverage.
        let properties = crate::inspect_database_properties(
            &mut Cursor::new(&bytes),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .unwrap();
        assert_eq!(properties.groups()[0].properties().len(), 2);
        let result = capture(&bytes).plan_table_rename("Items", "Items", &mut budget(), &|| false);
        assert!(matches!(result, Err(TableRenameError::Source)));
    }
}
