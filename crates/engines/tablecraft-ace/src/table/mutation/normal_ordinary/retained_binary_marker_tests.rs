//! Literal plans enter only independent ordinary proof, never staging/planning.
use super::proof;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan, NativeImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_retained_binary_marker::{
    self as source, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(238)
}

#[test]
#[ignore = "requires six independently qualified retained Binary Marker images"]
fn literal_marker_plans_replay_complete_images_and_refuse_protected_bytes_counts_and_witnesses() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut positives = 0;
    let mut forgeries = 0;
    let mut stale_witnesses = 0;
    for case in &cases {
        let orders: &[bool] = if case.recipe == Recipe::Batch {
            &[false, true]
        } else {
            &[false]
        };
        for &reverse in orders {
            let updates: Vec<_> = case
                .updates(reverse)
                .into_iter()
                .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
                .collect();
            let groups = [FixedColumnUpdates::new("BinaryKeys", "Marker", &updates)];
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: case.changed(),
            };
            proof::prove(
                &plan,
                scope(),
                "BinaryKeys",
                &groups,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            positives += 1;
        }
        let updates: Vec<_> = case
            .updates(false)
            .into_iter()
            .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
            .collect();
        let groups = [FixedColumnUpdates::new("BinaryKeys", "Marker", &updates)];
        for offset in case.forged_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: case.changed(),
            };
            assert_eq!(
                proof::prove(
                    &plan,
                    scope(),
                    "BinaryKeys",
                    &groups,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            forgeries += 1;
        }
        let wrong_count = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed() + 1,
        };
        assert_eq!(
            proof::prove(
                &wrong_count,
                scope(),
                "BinaryKeys",
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        // Noop and named-empty yield the same literal wrong-count plan here.
        // Their principal requests remain distinct; deduplicate only this stale witness.
        if case.recipe != Recipe::Empty {
            let stale = [FixedScalarUpdate::new(
                0,
                tablecraft_core::values::Value::Int32(18),
                tablecraft_core::values::Value::Int32(29),
            )];
            let groups = [FixedColumnUpdates::new("BinaryKeys", "Marker", &stale)];
            assert_eq!(
                proof::prove(
                    &wrong_count,
                    scope(),
                    "BinaryKeys",
                    &groups,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Conflict)
            );
            stale_witnesses += 1;
        }
    }
    assert_eq!(positives, 10);
    assert_eq!(forgeries, 176);
    assert_eq!(stale_witnesses, 6);
}

#[test]
#[ignore = "requires independently qualified original Binary schemas"]
fn action_certificate_cannot_substitute_for_sealed_canonical_update_source() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    for case in cases.iter().filter(|case| case.recipe == Recipe::Batch) {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let schema = crate::inspect_table_schema(
            &mut image.reader(),
            scope(),
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let actions =
            crate::table::mutation::no_required_programs::NoRequiredTablePrograms::capture_actions(
                &image,
                &schema,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(matches!(
            crate::table::mutation::preserved_source::capture_no_required_updates(
                &image,
                "BinaryKeys",
                &actions,
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Preservation)
        ));
        // The actual canonical route obtains its own declaration proof.
        super::capture(&image, "BinaryKeys", &mut budget(), &|| false).unwrap();
    }
}
