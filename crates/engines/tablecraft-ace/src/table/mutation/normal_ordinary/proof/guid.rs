//! Literal native GUID spans enter fresh proof without producer preparation.
use super::prove_guid;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_guid_update as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires qualified exact GUID originals and literal in-memory spans"]
fn guid_native_permutation_and_inactive_null_payload_have_independent_full_image_proof() {
    let cases = source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        );
    });
    let mut counts = [0; 3];
    for case in cases {
        let (row, before, after) = case.request();
        let updates = [FixedScalarUpdate::new(row, before, after)];
        let groups = [FixedColumnUpdates::new("Items", "GuidValue", &updates)];
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        prove_guid(
            &plan,
            CatalogScope::new(298),
            "Items",
            &groups,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        counts[0] += 1;
        for (offset, mask) in case.forgeries() {
            let mut after = case.after.clone();
            after[offset] ^= mask;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: 1,
            };
            assert_eq!(
                prove_guid(
                    &forged,
                    CatalogScope::new(298),
                    "Items",
                    &groups,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            counts[1] += 1;
        }
        let count = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 2,
        };
        assert_eq!(
            prove_guid(
                &count,
                CatalogScope::new(298),
                "Items",
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        counts[2] += 1;
    }
    assert_eq!(counts, [6, 72, 6]);
}
