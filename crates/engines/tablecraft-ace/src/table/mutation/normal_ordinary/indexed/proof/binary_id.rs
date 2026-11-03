//! Direct literal image proof through the real indexed proof owner only.
use super::prove;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_binary_id_update::{self as source, Recipe};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires four independently qualified Binary Id source images"]
fn literal_id_primary_replay_rejects_other_bytes_wrong_counts_and_stale_requests() {
    let cases = source::load_all(|raw, hash| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, hash);
    });
    let mut positives = 0;
    let mut byte_refusals = 0;
    let mut count_refusals = 0;
    let mut stale_refusals = 0;
    for case in cases {
        let updates: Vec<_> = case
            .updates()
            .into_iter()
            .map(|(row, old, new)| FixedScalarUpdate::new(row, old, new))
            .collect();
        let groups = [FixedColumnUpdates::new("BinaryKeys", "Id", &updates)];
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed(),
        };
        let scope = CatalogScope::new(245);
        prove(&plan, scope, "BinaryKeys", &groups, &mut budget(), &|| {
            false
        })
        .unwrap();
        positives += 1;
        for offset in case.forged_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: case.changed(),
            };
            assert_eq!(
                prove(
                    &forged,
                    scope,
                    "BinaryKeys",
                    &groups,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            byte_refusals += 1;
        }
        let wrong = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.changed() + 1,
        };
        assert_eq!(
            prove(&wrong, scope, "BinaryKeys", &groups, &mut budget(), &|| {
                false
            }),
            Err(EditError::Preservation)
        );
        count_refusals += 1;
        // Replacing no-op and empty groups would make these stale inputs identical.
        if case.recipe != Recipe::Empty {
            let stale = [FixedScalarUpdate::new(0, Value::Int32(9), Value::Int32(14))];
            let groups = [FixedColumnUpdates::new("BinaryKeys", "Id", &stale)];
            assert_eq!(
                prove(&plan, scope, "BinaryKeys", &groups, &mut budget(), &|| {
                    false
                }),
                Err(EditError::Conflict)
            );
            stale_refusals += 1;
        }
    }
    assert_eq!(
        (positives, byte_refusals, count_refusals, stale_refusals),
        (6, 150, 6, 4)
    );
}
