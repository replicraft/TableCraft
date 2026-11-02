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
    let mut cases = sources::load_all(|bytes, expected| {
        let actual: String = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    });
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_primary_prefix_growth::load_all(
            |bytes, expected| {
                let actual: String = Sha256::digest(bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                assert_eq!(actual, expected);
            },
        ),
    );
    cases.extend(
        tablecraft_testkit::fixtures::access::genuine_wide_leaf_compression::load_all(
            |bytes, expected| {
                let actual: String = Sha256::digest(bytes)
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                assert_eq!(actual, expected);
            },
        ),
    );
    for case in &cases {
        case.assert_literal_layout();
    }
    cases
}

#[test]
#[ignore = "requires four independently qualified literal wider promotion AFTERs"]
fn literal_wider_root_promotions_prove_without_stage_or_prepared_index_facts() {
    let cases = cases();
    assert_eq!(cases.len(), 12);
    for case in cases {
        let requests: Vec<_> = case
            .insert_values()
            .into_iter()
            .map(crate::RowInsert::new)
            .collect();
        let plan = NativeEditPlan {
            before: case.before,
            after: case.after,
            changed_rows: case.inserted,
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
        let requests: Vec<_> = case
            .insert_values()
            .into_iter()
            .map(crate::RowInsert::new)
            .collect();
        for offset in case.forgery_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: case.inserted,
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
        for count in [0, case.inserted + 1] {
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
        let mut wrong = case.insert_values();
        wrong[0][2] = tablecraft_core::values::Value::Int32(6818);
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: case.inserted,
        };
        assert_eq!(
            super::proof::prove(
                &plan,
                SCOPE,
                case.table,
                &wrong
                    .into_iter()
                    .map(crate::RowInsert::new)
                    .collect::<Vec<_>>(),
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
    }
    assert!(controls >= 240);
}
