//! Literal whole images enter only fresh original-presence proof.
use super::proof;
use crate::{InsertFieldValue, NativeEditPlan, OmittedRowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_increment_companions as fixtures;

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn requests(values: Vec<Option<Value>>) -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(
        values
            .into_iter()
            .map(|value| value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied))
            .collect(),
    )]
}

#[test]
#[ignore = "requires all independently qualified companion images; no planner or serializer"]
fn literal_companion_images_prove_and_protected_bytes_counts_and_intent_refuse() {
    let cases = fixtures::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    });
    let mut accepted = 0;
    let mut denied = 0;

    for case in &cases {
        let original = requests(case.presence());
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        proof::prove(
            &plan,
            CatalogScope::new(237),
            case.table,
            &original,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        accepted += 1;

        let empty = NativeEditPlan {
            before: case.before.clone(),
            after: case.before.clone(),
            changed_rows: 0,
        };
        proof::prove(
            &empty,
            CatalogScope::new(237),
            case.table,
            &[],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        accepted += 1;

        for offset in case.protected_offsets() {
            let mut forged = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: 1,
            };
            forged.after[offset] ^= 1;
            assert!(
                proof::prove(
                    &forged,
                    CatalogScope::new(237),
                    case.table,
                    &original,
                    &mut budget(),
                    &|| false,
                )
                .is_err()
            );
            denied += 1;
        }

        let wrong_count = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        assert!(
            proof::prove(
                &wrong_count,
                CatalogScope::new(237),
                case.table,
                &original,
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        denied += 1;

        let mut alternate = case.presence();
        alternate[2] = Some(if case.family == fixtures::Family::Fixed {
            Value::Int64(-8)
        } else {
            Value::Int32(18)
        });
        assert!(
            proof::prove(
                &plan,
                CatalogScope::new(237),
                case.table,
                &requests(alternate),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
        denied += 1;
    }

    assert_eq!(accepted, 40);
    assert_eq!(denied, 208);
}
