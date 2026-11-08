//! Literal images enter fresh independent proof without staging or serialization.
use super::proof;
use crate::{InsertFieldValue, NativeEditPlan, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_increment_multi as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn requests(presence: Vec<Vec<Option<Value>>>) -> Vec<OmittedRowInsert> {
    presence
        .into_iter()
        .map(|row| {
            OmittedRowInsert::new(
                row.into_iter()
                    .map(|value| {
                        value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            )
        })
        .collect()
}

#[test]
#[ignore = "requires four qualified source images; no planner/serializer/stage"]
fn literal_batch_proof_binds_request_order_generated_ids_two_pages_and_every_alias() {
    let cases = fixtures::load_all(|raw, wanted| {
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
    for case in &cases {
        let original = requests(fixtures::presence());
        let plan = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: 2,
        };
        proof::prove(
            &plan,
            CatalogScope::new(241),
            case.table,
            &original,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let empty = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.before.clone(),
            changed_rows: 0,
        };
        proof::prove(
            &empty,
            CatalogScope::new(241),
            case.table,
            &[],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        accepted += 2;

        for offset in case.protected_offsets() {
            let mut forged = NativeEditPlan {
                before: case.before.clone().into(),
                after: case.after.clone(),
                changed_rows: 2,
            };
            forged.after[offset] ^= 1;
            assert!(
                proof::prove(
                    &forged,
                    CatalogScope::new(241),
                    case.table,
                    &original,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            denied += 1;
        }
        for count in [0, 1, 3] {
            let wrong = NativeEditPlan {
                before: case.before.clone().into(),
                after: case.after.clone(),
                changed_rows: count,
            };
            assert!(
                proof::prove(
                    &wrong,
                    CatalogScope::new(241),
                    case.table,
                    &original,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            denied += 1;
        }
        let mut reversed = fixtures::presence();
        reversed.reverse();
        let mut wrong_score = fixtures::presence();
        wrong_score[1][1] = Some(Value::Int32(19));
        for presence in [reversed, wrong_score] {
            assert!(
                proof::prove(
                    &plan,
                    CatalogScope::new(241),
                    case.table,
                    &requests(presence),
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            denied += 1;
        }
        let mut changed_before = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: 2,
        };
        let mut altered_before = changed_before.before.to_vec();
        altered_before[case.definition * 4096 + 20..case.definition * 4096 + 24]
            .copy_from_slice(&136_i32.to_le_bytes());
        changed_before.before = altered_before.into();
        assert!(
            proof::prove(
                &changed_before,
                CatalogScope::new(241),
                case.table,
                &original,
                &mut budget(),
                &|| false
            )
            .is_err()
        );
        denied += 1;
    }
    assert_eq!(accepted, 4);
    assert_eq!(
        denied,
        cases
            .iter()
            .map(|case| case.protected_offsets().len() + 6)
            .sum::<usize>()
    );
}
