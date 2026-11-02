//! Literal plans only: independent proof sees no stage, encoder or planner output.
use crate::{EditError, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_wide_root_promotion::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(288);

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|bytes, expected| {
        let actual: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    })
}

#[test]
#[ignore = "requires four independently qualified literal wider promotion AFTERs"]
fn literal_wider_root_promotions_prove_without_stage_or_prepared_index_facts() {
    let cases = cases();
    assert_eq!(cases.len(), 4);
    for case in cases {
        let requests = [crate::RowInsert::new(case.values())];
        let plan = NativeEditPlan {
            before: case.before,
            after: case.after,
            changed_rows: 1,
        };
        super::proof::prove(&plan, SCOPE, case.table, &requests, &mut budget(), &|| {
            false
        })
        .unwrap();
    }
}

#[test]
#[ignore = "requires literal wider promotion images for independent proof corruption checks"]
fn manual_wider_proof_rejects_children_row_partition_history_masks_and_opaque_residuals() {
    let mut controls = 0;
    for case in cases() {
        let requests = [crate::RowInsert::new(case.values())];
        for offset in case.forgery_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: 1,
            };
            assert_eq!(
                super::proof::prove(&plan, SCOPE, case.table, &requests, &mut budget(), &|| {
                    false
                }),
                Err(EditError::Preservation),
                "{} h{} byte{offset}",
                case.table,
                case.header
            );
            controls += 1;
        }
        for count in [0, 2] {
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after: case.after.clone(),
                changed_rows: count,
            };
            assert_eq!(
                super::proof::prove(&plan, SCOPE, case.table, &requests, &mut budget(), &|| {
                    false
                }),
                Err(EditError::Preservation)
            );
        }
        let mut wrong = case.values();
        wrong[2] = tablecraft_core::values::Value::Int32(6818);
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        assert_eq!(
            super::proof::prove(
                &plan,
                SCOPE,
                case.table,
                &[crate::RowInsert::new(wrong)],
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
    assert!(controls >= 240);
}
