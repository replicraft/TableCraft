//! Independent literal Byte/Int16 payload and inactive-NULL replay.
use super::prove_scalar;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_byte_int16_update as source;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires qualified exact originals and independent in-memory Byte/Int16 spans"]
fn byte_int16_literal_spans_and_null_inactive_bytes_are_independently_proved() {
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
        let requests = case.requests();
        let updates: Vec<_> = requests
            .iter()
            .map(|(_, before, after)| {
                vec![FixedScalarUpdate::new(2, before.clone(), after.clone())]
            })
            .collect();
        let groups: Vec<_> = requests
            .iter()
            .zip(&updates)
            .map(|((column, _, _), updates)| FixedColumnUpdates::new("Items", column, updates))
            .collect();
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        prove_scalar(
            &plan,
            CatalogScope::new(295),
            "Items",
            &groups,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        counts[0] += 1;
        for offset in case.forged_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: 1,
            };
            assert_eq!(
                prove_scalar(
                    &forged,
                    CatalogScope::new(295),
                    "Items",
                    &groups,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            counts[1] += 1;
        }
        let forged = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 2,
        };
        assert_eq!(
            prove_scalar(
                &forged,
                CatalogScope::new(295),
                "Items",
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        counts[2] += 1;
    }
    assert_eq!(counts, [6, 60, 6]);
}
