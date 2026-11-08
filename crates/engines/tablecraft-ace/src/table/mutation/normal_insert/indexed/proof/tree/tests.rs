//! Direct literal plans: no insertion producer supplies a proof expectation.
use super::super::prove;
use crate::{EditError, NativeEditPlan, RowInsert};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_retained_node_insert as sources;

const SCOPE: CatalogScope = CatalogScope::new(192);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<sources::Case> {
    sources::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

#[test]
#[ignore = "requires four source-first NODE INSERT images and native/SDK24 qualification"]
fn independent_node_proof_accepts_literal_growth_and_original_empty_images() {
    let mut proved = 0;
    for case in cases() {
        for empty in [false, true] {
            let plan = NativeEditPlan {
                before: case.before.clone().into(),
                after: if empty {
                    case.before.clone()
                } else {
                    case.after.clone()
                },
                changed_rows: usize::from(!empty),
            };
            let requests = if empty {
                vec![]
            } else {
                vec![RowInsert::new(sources::values())]
            };
            prove(&plan, SCOPE, "Items", &requests, &mut budget(), &|| false).unwrap();
            proved += 1;
        }
    }
    assert_eq!(proved, 4);
}

#[test]
#[ignore = "requires four source-first NODE INSERT images and native/SDK24 qualification"]
fn independent_node_proof_rejects_fence_row_partition_history_and_inactive_complement_changes() {
    let mut refused = 0;
    for case in cases() {
        for offset in [
            case.roots[1] * 4096 + 481,
            case.roots[1] * 4096 + 485,
            case.roots[1] * 4096 + 489,
            case.left[1] * 4096 + 27,
            case.right[0] * 4096 + 480,
            case.left[1] * 4096 + 4095,
            case.definition * 4096 + 67,
            case.definition * 4096 + 79,
            case.data * 4096 + 2895,
        ] {
            let mut changed = case.after.clone();
            changed[offset] ^= 1;
            let plan = NativeEditPlan {
                before: case.before.clone().into(),
                after: changed,
                changed_rows: 1,
            };
            assert!(matches!(
                prove(
                    &plan,
                    SCOPE,
                    "Items",
                    &[RowInsert::new(sources::values())],
                    &mut budget(),
                    &|| false
                ),
                Err(EditError::Preservation)
            ));
            refused += 1;
        }
        let plan = NativeEditPlan {
            before: case.before.into(),
            after: case.after,
            changed_rows: 0,
        };
        assert!(matches!(
            prove(
                &plan,
                SCOPE,
                "Items",
                &[RowInsert::new(sources::values())],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        ));
        refused += 1;
    }
    assert_eq!(refused, 20);
}
