//! Independent complete images and forbidden effects for retained table deletion.
use super::*;
use crate::{
    format::page::PAGE_BYTES,
    inspect_catalog,
    table::mutation::drop_table::{row_effects, source},
};
use std::io::Cursor;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_table_drop::retained::{self as raw, CASES, Case},
    raw_forwarding_chain,
};

const SCOPE: CatalogScope = CatalogScope::new(3901);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first differing byte {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b)
    );
}

fn check_drop(before: &[u8], definition: u32, root: usize, roots: &[usize]) {
    let expected = raw::after(before, definition, root, roots);
    let native = image(before);
    let inspection = native
        .inspect_table_drop_source("table1", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(inspection.table(), "Table1");
    assert_eq!(inspection.identity().definition_page(), definition);
    assert_eq!(usize::from(inspection.identity().catalog_slot()), root);
    assert_eq!(inspection.declared_rows() as usize, roots.len());

    let plan = native
        .plan_table_drop("table1", &mut budget(), &|| false)
        .unwrap();
    assert_eq!(plan.source_profile(), SourceProfile::Retained);
    equal_image(plan.before_bytes(), before);
    equal_image(plan.after_bytes(), &expected);
    let fresh = inspect_catalog(
        &mut Cursor::new(plan.after_bytes()),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        fresh
            .catalog()
            .objects()
            .iter()
            .all(|object| { object.name().as_str() != "Table1" })
    );
    assert!(
        fresh
            .catalog()
            .objects()
            .iter()
            .any(|object| { object.name().as_str() == "Other" })
    );
    assert!(matches!(
        image(plan.after_bytes()).inspect_table_drop_source("Table1", &mut budget(), &|| false),
        Err(TableDropError::MissingTable)
    ));
}

#[test]
fn retained_drop_matches_complete_images_for_appended_sparse_empty_and_forwarded_tables() {
    for case in CASES {
        let source = raw::source(case);
        check_drop(
            &source.bytes,
            source.definition,
            source.catalog_root,
            &source.roots,
        );
    }
}

#[test]
fn retained_drop_preserves_every_byte_outside_row_index_and_catalog_actions() {
    for case in CASES {
        let raw = raw::source(case);
        let native = image(&raw.bytes);
        let source = qualify(&native, "Table1", &mut budget(), &|| false).unwrap();
        let expected = raw::after(&raw.bytes, raw.definition, raw.catalog_root, &raw.roots);
        row_effects::candidate(&native, &expected, &source, &mut budget(), &|| false).unwrap();

        // Header, original TDEF slack, retained record payload and a sibling's
        // allocated page are outside both genuine row and catalog edits.
        let sibling = if raw.definition == 11 { 14 } else { 11 };
        for offset in [
            100,
            raw.definition as usize * PAGE_BYTES + PAGE_BYTES - 1,
            (raw.definition as usize + 1) * PAGE_BYTES + PAGE_BYTES - 1,
            sibling * PAGE_BYTES + PAGE_BYTES - 1,
        ] {
            let mut forged = expected.to_vec();
            forged[offset] ^= 1;
            assert!(matches!(
                row_effects::candidate(&native, &forged, &source, &mut budget(), &|| false),
                Err(TableDropError::Preservation)
            ));
        }
        for older in [
            SourceProfile::Inline,
            SourceProfile::CreatorExternal,
            SourceProfile::Current,
        ] {
            assert!(source::qualify(&native, "Table1", older, &mut budget(), &|| false).is_err());
        }
    }
}

#[test]
fn retained_drop_requires_owned_acyclic_history_and_actual_empty_dependency_tables() {
    let source = raw::source(Case::FirstNumber);
    let user = raw_forwarding_chain::extend(&source.bytes, source.definition + 1, 0, 20);
    let both = raw_forwarding_chain::extend(&user, 4, source.catalog_root, 20);
    check_drop(&both, source.definition, source.catalog_root, &source.roots);

    let data = (source.definition as usize + 1) * 4096;
    let count = usize::from(u16::from_le_bytes(
        user[data + 12..data + 14].try_into().unwrap(),
    ));
    let entry = data + 14 + (count - 1) * 2;
    let terminal = u16::from_le_bytes(user[entry..entry + 2].try_into().unwrap());
    let at = data + usize::from(terminal & 0x1fff);
    let mut cycle = user;
    cycle[entry..entry + 2].copy_from_slice(&(terminal | 0x4000).to_le_bytes());
    cycle[at..at + 4].copy_from_slice(&((source.definition + 1) << 8).to_le_bytes());
    assert!(
        image(&cycle)
            .plan_table_drop("Table1", &mut budget(), &|| false)
            .is_err()
    );

    let mut wrong_owner = both.to_vec();
    wrong_owner[data + 4..data + 8].copy_from_slice(&7u32.to_le_bytes());
    assert!(
        image(&wrong_owner)
            .plan_table_drop("Table1", &mut budget(), &|| false)
            .is_err()
    );

    for definition in [5, 6] {
        let mut declared = source.bytes.to_vec();
        let at = definition * PAGE_BYTES + 16;
        declared[at..at + 4].copy_from_slice(&1u32.to_le_bytes());
        assert!(
            image(&declared)
                .plan_table_drop("Table1", &mut budget(), &|| false)
                .is_err()
        );
    }
    assert!(matches!(
        image(&both).plan_table_drop("Table1", &mut budget(), &|| true),
        Err(TableDropError::Inspect(InspectError::Cancelled))
    ));
    assert!(
        image(&both)
            .plan_table_drop("Table1", &mut ResourceBudget::new(0, 0, 0), &|| false)
            .is_err()
    );
}
