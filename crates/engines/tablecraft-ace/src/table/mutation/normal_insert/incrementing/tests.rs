//! Independent literal AFTERs enter only the generated original-intent proof.
use super::proof;
use crate::{EditError, InsertFieldValue, NativeEditPlan, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_normal_autonumber as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn requests() -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Int32(17)),
    ])]
}

#[test]
#[ignore = "requires14 independent source views; no producer/staging/serializer"]
fn literal_increment_images_and_empty_requests_prove_from_original_presence() {
    let sources = fixtures::Sources::load(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    });
    let mut accepted = 0;
    let mut denied = 0;
    for header in [5, 6] {
        for recipe in fixtures::RECIPES {
            let before = sources.bytes(header, recipe.before());
            let after = sources.bytes(header, recipe.after());
            let plan = NativeEditPlan {
                before: before.to_vec(),
                after: after.to_vec(),
                changed_rows: 1,
            };
            proof::prove(
                &plan,
                CatalogScope::new(233),
                "Items",
                &requests(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            accepted += 1;
            let empty = NativeEditPlan {
                before: before.to_vec(),
                after: before.to_vec(),
                changed_rows: 0,
            };
            proof::prove(
                &empty,
                CatalogScope::new(233),
                "Items",
                &[],
                &mut budget(),
                &|| false,
            )
            .unwrap();
            accepted += 1;
            let new_start = match recipe {
                fixtures::Recipe::High => 4074,
                fixtures::Recipe::Zero => 4085,
            };
            for offset in [
                7 * 4096 + 20,
                7 * 4096 + 67,
                8 * 4096 + new_start + 2,
                8 * 4096 + new_start + 6,
                8 * 4096 + new_start + 10,
                9 * 4096 + 480,
                9 * 4096 + 4095,
                3 * 4096 + 4080,
            ] {
                let mut forged = NativeEditPlan {
                    before: before.to_vec(),
                    after: after.to_vec(),
                    changed_rows: 1,
                };
                forged.after[offset] ^= 1;
                assert_eq!(
                    proof::prove(
                        &forged,
                        CatalogScope::new(233),
                        "Items",
                        &requests(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(EditError::Preservation)
                );
                denied += 1;
            }
            let count = NativeEditPlan {
                before: before.to_vec(),
                after: after.to_vec(),
                changed_rows: 0,
            };
            assert_eq!(
                proof::prove(
                    &count,
                    CatalogScope::new(233),
                    "Items",
                    &requests(),
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            denied += 1;
            let wrong = vec![OmittedRowInsert::new(vec![
                InsertFieldValue::Omitted,
                InsertFieldValue::Supplied(Value::Int32(18)),
            ])];
            assert_eq!(
                proof::prove(
                    &plan,
                    CatalogScope::new(233),
                    "Items",
                    &wrong,
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            );
            denied += 1;
        }
    }
    assert_eq!(accepted, 8);
    assert_eq!(denied, 40);
}
