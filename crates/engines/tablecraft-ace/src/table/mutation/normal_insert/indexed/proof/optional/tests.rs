//! Direct literal NativeEditPlan proof: no planner/staging function supplies bytes.
use super::super::prove;
use crate::{EditError, NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_desc_nullable_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(182);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<sources::Case> {
    sources::load_all(|raw, expected| {
        let hash = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    })
}

fn requests(count: usize) -> Vec<RowInsert> {
    (0..count)
        .map(|position| RowInsert::new(sources::values(position == 1)))
        .collect()
}

#[test]
#[ignore = "requires source native6 and SDK36 nullable/DESC INSERT qualification"]
fn four_literal_optional_directional_images_prove_without_insertion_stage_authority() {
    let mut proved = 0;
    for case in cases() {
        for (count, after) in [(1, &case.single), (2, &case.batch)] {
            let plan = NativeEditPlan {
                before: case.before.clone().into(),
                after: after.clone(),
                changed_rows: count,
            };
            prove(
                &plan,
                SCOPE,
                "Items",
                &requests(count),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            proved += 1;
        }
    }
    assert_eq!(proved, 4);
}

#[test]
#[ignore = "requires source native6 and SDK36 nullable/DESC INSERT qualification"]
fn direct_optional_proof_rejects_direction_null_history_rows_and_original_carrier_changes() {
    let mut refused = 0;
    for case in cases() {
        for (count, after) in [(1, &case.single), (2, &case.batch)] {
            let guard = case.roots[1] * 4096;
            let null_at = guard + if count == 1 { 503 } else { 512 };
            assert_eq!(after[null_at], 255);
            for offset in [
                null_at,
                null_at + 4,
                case.definition * 4096 + 79,
                case.data * 4096 + 4046,
                case.data * 4096 + 4050,
                case.data * 4096 + 4091,
            ] {
                let mut changed = after.clone();
                if offset == null_at {
                    changed[offset] = 0;
                } else {
                    changed[offset] ^= 1;
                }
                let forged = NativeEditPlan {
                    before: case.before.clone().into(),
                    after: changed,
                    changed_rows: count,
                };
                assert!(matches!(
                    prove(
                        &forged,
                        SCOPE,
                        "Items",
                        &requests(count),
                        &mut budget(),
                        &|| false
                    ),
                    Err(EditError::Preservation)
                ));
                refused += 1;
            }
            for (changed, changed_rows) in [
                (after.clone(), 0),
                (after[..after.len() - 1].to_vec(), count),
            ] {
                let forged = NativeEditPlan {
                    before: case.before.clone().into(),
                    after: changed,
                    changed_rows,
                };
                assert!(matches!(
                    prove(
                        &forged,
                        SCOPE,
                        "Items",
                        &requests(count),
                        &mut budget(),
                        &|| false
                    ),
                    Err(EditError::Preservation)
                ));
                refused += 1;
            }
            let mut original = (0..count)
                .map(|position| sources::values(position == 1))
                .collect::<Vec<_>>();
            original[0][2] = Value::Int32(0);
            let changed_request = original.into_iter().map(RowInsert::new).collect::<Vec<_>>();
            let plan = NativeEditPlan {
                before: case.before.clone().into(),
                after: after.clone(),
                changed_rows: count,
            };
            // This is a valid DIFFERENT request; it must reconstruct a different
            // image, not acquire the NULL-bearing literal image as authority.
            assert!(matches!(
                prove(
                    &plan,
                    SCOPE,
                    "Items",
                    &changed_request,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
            refused += 1;
        }
    }
    assert_eq!(refused, 36);
}
