//! Literal source plans are proved without any producer, serializer or staging.
use super::proof;
use crate::{EditError, NativeEditPlan, RowDelete};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_node_retirement::{self as fixtures, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    fixtures::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn plan(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone(),
        after: case.after.clone(),
        changed_rows: case.recipe.selected().len(),
    }
}

fn requests(case: &Case, reverse: bool) -> Vec<RowDelete> {
    let mut ordinals = case.recipe.selected().collect::<Vec<_>>();
    if reverse {
        ordinals.reverse();
    }
    ordinals
        .into_iter()
        .map(|ordinal| RowDelete::new(ordinal, fixtures::record(ordinal)))
        .collect()
}

fn prove(plan: &NativeEditPlan, rows: &[RowDelete]) -> Result<(), EditError> {
    proof::prove(
        plan,
        CatalogScope::new(259),
        "Items",
        rows,
        &mut budget(),
        &|| false,
    )
}

#[test]
#[ignore = "requires eight native/SDK-qualified NODE retirement source views"]
fn literal_node_retirement_proof_accepts_both_orders_and_exact_empty_after_sources() {
    let mut accepted = 0;
    for case in cases() {
        for reverse in [false, true] {
            prove(&plan(&case), &requests(&case, reverse)).unwrap();
            accepted += 1;
        }
        let empty = NativeEditPlan {
            before: case.after.clone(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        prove(&empty, &[]).unwrap();
        accepted += 1;
    }
    assert_eq!(accepted, 18);
}

#[test]
#[ignore = "requires eight native/SDK-qualified NODE retirement source views"]
fn direct_proof_rejects_fence_row_map_retired_residual_history_and_data_changes() {
    let mut refused = 0;
    for case in cases() {
        let requests = requests(&case, false);
        for offset in fixtures::forged_offsets(&case) {
            let mut forged = plan(&case);
            forged.after[offset] ^= 1;
            assert_eq!(prove(&forged, &requests), Err(EditError::Preservation));
            refused += 1;
        }
        let mut count = plan(&case);
        count.changed_rows += 1;
        assert_eq!(prove(&count, &requests), Err(EditError::Preservation));
        refused += 1;

        // An independently changed, still opaque original tail is part of the
        // actual source authority, not replaceable by an expected-image hash.
        let mut original = plan(&case);
        original.before[case.left[0] * 4096 + 4095] ^= 1;
        assert_eq!(prove(&original, &requests), Err(EditError::Preservation));
        refused += 1;
    }
    assert!(refused >= 150);
}
