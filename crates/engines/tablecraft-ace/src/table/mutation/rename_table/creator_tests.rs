//! Independent complete-image qualification for explicit creator OFF sources.
use crate::{NativeImage, TableRenameError, inspect_table_properties, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_creation,
    canonical_creator_rename::{originals, pair, with_text_property},
    canonical_native17_creation,
};

const SCOPE: CatalogScope = CatalogScope::new(747);
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

#[test]
fn creator_off_candidates_match_complete_independent_images_and_preserve_field_facts() {
    for original in originals() {
        let explicit_off = capture(&original)
            .plan_disable_name_auto_correct(&mut budget(), &|| false)
            .unwrap();
        assert_eq!(explicit_off.after_bytes(), pair(&original, "Items").before);
        for name in [
            "Inventory",
            "I",
            "items",
            "Items",
            "雪🙂",
            "\u{feff}Escaped",
        ] {
            let expected = pair(&original, name);
            let before = read_table(
                &mut Cursor::new(&expected.before),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let properties = inspect_table_properties(
                &mut Cursor::new(&expected.before),
                SCOPE,
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let plan = capture(&expected.before)
                .plan_creator_table_rename("Items", name, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.before_bytes(), expected.before);
            assert_eq!(plan.after_bytes(), expected.after);
            assert_eq!(plan.changed(), name != "Items");
            assert_eq!(plan.identity().catalog_slot(), 7);
            assert_eq!(plan.identity().native_id(), 7);
            let after = read_table(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(after.rows(), before.rows());
            let retained = inspect_table_properties(
                &mut Cursor::new(plan.after_bytes()),
                SCOPE,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(properties.raw_bytes(), retained.raw_bytes());
            for page in 0..expected.before.len() / PAGE {
                if page != 4 {
                    assert_eq!(
                        &expected.before[page * PAGE..(page + 1) * PAGE],
                        &expected.after[page * PAGE..(page + 1) * PAGE]
                    );
                }
            }
        }
    }
}

#[test]
fn creator_off_is_an_explicit_distinct_source_not_an_old_profile_or_default() {
    for original in originals() {
        assert!(
            capture(&original)
                .plan_creator_table_rename("Items", "Items", &mut budget(), &|| false)
                .is_err()
        );
        let expected = pair(&original, "Inventory");
        assert!(
            capture(&expected.before)
                .plan_table_rename("Items", "Inventory", &mut budget(), &|| false)
                .is_err()
        );
        assert!(
            capture(&expected.before)
                .plan_indexed_table_rename("Items", "Inventory", &mut budget(), &|| false)
                .is_err()
        );
    }
}

#[test]
fn creator_field_properties_refuse_programs_unknown_metadata_and_name_references_even_for_noops() {
    let ordinary = canonical_native17_creation::source(
        canonical_creation::source(canonical_creation::Case::One).bytes,
    );
    for (column, key, kind, value) in [
        ("Score", "DefaultValue", 12, "Now()"),
        ("Score", "DefaultValue", 12, "[Items].[Score]"),
        ("Score", "DefaultValue", 12, "1"),
        ("Score", "ValidationRule", 12, ">=0"),
        ("Score", "Description", 12, "wrong native carrier"),
        ("Missing", "Description", 10, "unknown field"),
        ("Score", "NameMap", 10, "Items"),
    ] {
        let original = with_text_property(ordinary.clone(), column, key, kind, value);
        let expected = pair(&original, "Items");
        assert!(matches!(
            capture(&expected.before).plan_creator_table_rename(
                "Items",
                "Items",
                &mut budget(),
                &|| false
            ),
            Err(TableRenameError::Dependency)
        ));
    }
}

#[test]
fn creator_external_ownership_allocation_and_off_value_guards_apply_before_noops() {
    let expected = pair(&originals().remove(0), "Items");
    let mut cases = Vec::new();
    let eof = expected.before.len() - PAGE;
    let mut flagged = expected.before.clone();
    flagged[eof + 15] |= 0x80;
    cases.push(flagged);
    let mut enabled = expected.before.clone();
    let end = enabled.len();
    enabled[end - 4..].copy_from_slice(&1_i32.to_le_bytes());
    cases.push(enabled);
    let mut owner = expected.before.clone();
    owner[eof + 4..eof + 8].copy_from_slice(&7_u32.to_le_bytes());
    cases.push(owner);
    let mut membership = expected.before.clone();
    let at = 3 * PAGE + 14 + 2 * 17;
    let start = usize::from(u16::from_le_bytes(
        membership[at..at + 2].try_into().unwrap(),
    ));
    membership[3 * PAGE + start + 6] = 0;
    cases.push(membership);
    for bytes in cases {
        assert!(
            capture(&bytes)
                .plan_creator_table_rename("Items", "Items", &mut budget(), &|| false)
                .is_err()
        );
    }
    assert!(
        capture(&expected.before)
            .plan_creator_table_rename("Items", "Items", &mut budget(), &|| true)
            .is_err()
    );
    assert!(
        capture(&expected.before)
            .plan_creator_table_rename("Items", "Items", &mut ResourceBudget::new(0, 0, 0), &|| {
                false
            })
            .is_err()
    );
}
