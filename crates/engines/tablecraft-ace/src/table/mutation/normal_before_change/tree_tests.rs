//! Independent retained-tree proof and explicit fitting-policy controls.
use super::proof;
use crate::{
    EditError, IndexCheckStatus, InspectError, Int32Update, NativeEditPlan, NativeImage,
    WriteBlocker, check_table,
    table::indexes::{
        IndexKeyUpdate, MutationKey, index_patches,
        mutation::{normal_fitting, normal_index_audit::Ownership},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;
use tablecraft_testkit::access::canonical_tree_before_change::{self as model, Case, Recipe};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(99)
}

fn updates(case: &Case) -> Vec<Int32Update> {
    case.updates
        .iter()
        .map(|update| Int32Update::new(update.ordinal, update.expected, update.replacement))
        .collect()
}

fn observation(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone(),
        after: case.expected.clone(),
        changed_rows: case.changed_rows,
    }
}

fn prove(plan: &NativeEditPlan, updates: &[Int32Update]) -> Result<(), EditError> {
    proof::prove(
        plan,
        scope(),
        "Items",
        "Score",
        updates,
        &mut budget(),
        &|| false,
    )
}

fn checked_source(bytes: &[u8]) -> (NativeImage, crate::TableCheck) {
    let image =
        NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap();
    let checked = check_table(
        &mut image.reader(),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(checked.rows(), 3);
    assert!(
        checked
            .indexes()
            .iter()
            .all(|index| index.status() == IndexCheckStatus::Checked)
    );
    (image, checked)
}

fn pointer(bytes: &[u8], page: usize, slot: usize) -> usize {
    let at = page * 4096 + 14 + 2 * slot;
    let word = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(word & 0xe000, 0);
    page * 4096 + usize::from(word)
}

#[test]
fn tree_proof_reconstructs_eighteen_literal_images_and_protects_nodes_prefixes_and_noops() {
    let mut positives = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            prove(&observation(&case), &updates(&case)).unwrap();
            positives += 1;
        }
    }
    assert_eq!(positives, 18);
    for recipe in [
        Recipe::NodePrefixLoss,
        Recipe::NodeZeroTail,
        Recipe::NodeRetainedUpper,
    ] {
        let case = model::source(6, recipe);
        for page in [9, 10, 11] {
            for relative in [2, 24, 27, 480, 4095] {
                let mut forged = observation(&case);
                forged.after[page * 4096 + relative] ^= 1;
                assert_eq!(
                    prove(&forged, &updates(&case)),
                    Err(EditError::Preservation)
                );
            }
        }
        for relative in [8, 20, 26, 488, 492] {
            let mut forged = observation(&case);
            forged.after[9 * 4096 + relative] ^= 1;
            assert_eq!(
                prove(&forged, &updates(&case)),
                Err(EditError::Preservation)
            );
        }
        let mut wrong_count = observation(&case);
        wrong_count.changed_rows += 1;
        assert_eq!(
            prove(&wrong_count, &updates(&case)),
            Err(EditError::Preservation)
        );
    }
    let case = model::source(6, Recipe::NodeAscending);
    assert_eq!(
        prove(&observation(&case), &[Int32Update::new(0, 10, 19)]),
        Err(EditError::Preservation)
    );
    assert_eq!(
        prove(&observation(&case), &[Int32Update::new(0, 11, 17)]),
        Err(EditError::Conflict)
    );
    let retained = model::source(6, Recipe::NodeRetainedUpper);
    let empty = NativeEditPlan {
        before: retained.before.clone(),
        after: retained.before.clone(),
        changed_rows: 0,
    };
    prove(&empty, &[]).unwrap();
}

#[test]
fn retained_tree_ownership_refuses_actual_leaf_overlap_global_free_and_node_stamp() {
    let case = model::source(6, Recipe::NodeAscending);
    let (healthy, checked) = checked_source(&case.before);
    let ownership = Ownership::capture(&healthy, &checked, &mut budget(), &|| false).unwrap();
    assert!(
        ownership
            .require(
                &healthy,
                checked.schema(),
                checked.indexes()[0].index(),
                &mut budget(),
                &|| false
            )
            .is_ok()
    );
    for conflict in 0..4 {
        let mut bytes = case.before.clone();
        if conflict == 0 {
            let catalog = 2 * 4096;
            assert_eq!(&bytes[catalog + 8..catalog + 12], &704_u32.to_le_bytes());
            let tail = catalog + 704 - 42;
            assert_eq!(&bytes[tail..tail + 2], &5_u16.to_le_bytes());
            assert_eq!(
                &bytes[tail + 2..tail + 6],
                &((3_u32 << 8) | 8).to_le_bytes()
            );
            bytes[tail + 2..tail + 6].copy_from_slice(&((3_u32 << 8) | 16).to_le_bytes());
        } else if conflict == 1 {
            let map = pointer(&bytes, 3, 8);
            assert_eq!(&bytes[map..map + 13], &[0; 13]);
            bytes[map + 6] = 4; // Catalog LVAL map also claims actual child10.
        } else if conflict == 2 {
            let global = pointer(&bytes, 1, 0);
            assert_eq!(&bytes[global..global + 13], &[0; 13]);
            bytes[global + 6] = 4; // Child10 is declared free in GLOBAL.
        } else {
            bytes[9 * 4096 + 8] = 1;
        }
        // Selected tree/rows still read completely; writable ownership is the
        // stricter refusal, not a forged cached schema or fake TableCheck.
        let (image, checked) = checked_source(&bytes);
        let error = Ownership::capture(&image, &checked, &mut budget(), &|| false)
            .err()
            .expect("unqualified source ownership");
        if conflict < 2 {
            assert_eq!(error, EditError::Unsupported(WriteBlocker::RowLayout));
        } else if conflict == 2 {
            assert_eq!(error, EditError::Inspect(InspectError::Malformed));
        } else {
            assert_eq!(error, EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        assert_eq!(image.as_bytes(), bytes);
    }
    // A different source contradiction fails earlier during the full tree read:
    // the header tail still names11, but the actual owned map omits that child.
    let mut missing = case.before.clone();
    let map = pointer(&missing, 3, 16);
    assert_eq!(missing[map + 6], 14);
    missing[map + 6] = 6;
    let image = NativeImage::capture(&mut Cursor::new(&missing), scope(), &mut budget(), &|| {
        false
    })
    .unwrap();
    assert!(
        check_table(
            &mut image.reader(),
            scope(),
            "Items",
            &mut budget(),
            &|| false
        )
        .is_err()
    );
}

#[test]
fn normal_fitting_is_a_distinct_preselected_service_and_preserves_typed_failures() {
    let kind = MutationKey::new(ScalarType::Int32).unwrap();
    for (recipe, row, old, final_value) in [
        (Recipe::LeafPrefixLoss, 0_u32, 10_i32, i32::MAX),
        (Recipe::NodePrefixLoss, 2_u32, 30_i32, -20_i32),
    ] {
        let case = model::source(6, recipe);
        let (image, checked) = checked_source(&case.before);
        let changes = [IndexKeyUpdate {
            reference: (8 << 8) | row,
            column: 1,
            expected: Some(kind.stored(&old.to_le_bytes()).unwrap()),
            value: Some(kind.stored(&final_value.to_le_bytes()).unwrap()),
        }];
        assert!(matches!(
            index_patches(
                &image,
                checked.schema(),
                checked.indexes(),
                &changes,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Unsupported(WriteBlocker::IndexedColumn))
        ));
        let patches =
            normal_fitting::patches(&image, &checked, &changes, &mut budget(), &|| false).unwrap();
        let mut actual = case.before.clone();
        for patch in patches {
            assert_eq!(patch.before.len(), patch.after.len());
            assert!(patch.offset >= 9 * 4096);
            let end = patch.offset.checked_add(patch.after.len()).unwrap();
            assert!(end <= actual.len());
            assert_eq!(&actual[patch.offset..end], patch.before);
            actual[patch.offset..end].copy_from_slice(&patch.after);
        }
        assert_eq!(&actual[..9 * 4096], &case.before[..9 * 4096]);
        assert_eq!(&actual[9 * 4096..], &case.expected[9 * 4096..]);
        assert_eq!(actual.len(), case.before.len());
        assert!(matches!(
            normal_fitting::patches(&image, &checked, &changes, &mut budget(), &|| true),
            Err(EditError::Inspect(InspectError::Cancelled))
        ));
        assert!(
            matches!(normal_fitting::patches(&image, &checked, &changes, &mut ResourceBudget::new(0, 1_000_000, 1_000_000_000), &|| false), Err(EditError::Inspect(InspectError::Limit(error))) if error.resource == tablecraft_core::limits::Resource::Bytes)
        );
        let stale = [IndexKeyUpdate {
            reference: (8 << 8) | row,
            column: 1,
            expected: Some(kind.stored(&(old + 1).to_le_bytes()).unwrap()),
            value: Some(kind.stored(&final_value.to_le_bytes()).unwrap()),
        }];
        assert!(matches!(
            normal_fitting::patches(&image, &checked, &stale, &mut budget(), &|| false),
            Err(EditError::Conflict)
        ));
    }
    let retained = model::source(6, Recipe::NodeRetainedUpper);
    let (image, checked) = checked_source(&retained.before);
    assert!(
        normal_fitting::patches(&image, &checked, &[], &mut budget(), &|| false)
            .unwrap()
            .is_empty()
    );
    assert_eq!(image.as_bytes(), retained.before);
}
