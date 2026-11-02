//! Literal source plans are proved without any producer, serializer or staging.
use super::proof;
use crate::{EditError, NativeEditPlan, RowDelete};
use sha2::{Digest, Sha256};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::genuine_recursive_retirement::{self as fixtures, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(2_000_000_000, 10_000_000, 2_000_000_000)
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
        "BinaryStorage",
        rows,
        &mut budget(),
        &|| false,
    )
}

#[test]
#[ignore = "requires six native/SDK-qualified recursive retirement source views"]
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
    assert_eq!(accepted, 12);
}

#[test]
#[ignore = "requires six native/SDK-qualified recursive retirement source views"]
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
        original.before[case.retired[0] as usize * 4096 + 4095] ^= 1;
        assert_eq!(prove(&original, &requests), Err(EditError::Preservation));
        refused += 1;
    }
    assert_eq!(refused, 84);
}

#[test]
#[ignore = "requires six qualified source images; authored optional-link convention control"]
fn recursive_proof_preserves_originally_unlinked_internal_levels() {
    // The existing strict tree reader explicitly allows an internal level whose
    // original previous/next fields are all zero. This is an authored boundary
    // control, not a new SDK-native AFTER compatibility claim. Every other byte
    // comes from the frozen original and corresponding independent source AFTER.
    for case in cases() {
        let mut literal = plan(&case);
        let other = if case.definition == 89 { 820 } else { 838 };
        for page in [case.parent, other] {
            for field in [12, 16] {
                let offset = page * 4096 + field;
                literal.before[offset..offset + 4].fill(0);
                literal.after[offset..offset + 4].fill(0);
            }
        }
        prove(&literal, &requests(&case, false)).unwrap();
    }
}
