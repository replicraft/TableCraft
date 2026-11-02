//! Literal complete plans reach the fresh proof without the physical producer.
use super::proof;
use crate::{EditError, FixedColumnUpdates, FixedScalarUpdate, NativeEditPlan};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_wide_update::{self as source, Case};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    source::load_all(|raw, wanted| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, wanted);
    })
}

fn original(case: &Case, reverse: bool) -> Vec<(&str, Vec<FixedScalarUpdate>)> {
    let mut groups: Vec<_> = case
        .groups
        .iter()
        .map(|(column, assignments)| {
            let mut values: Vec<_> = assignments
                .iter()
                .map(|assignment| {
                    FixedScalarUpdate::new(
                        assignment.row,
                        source::cell(&assignment.expected),
                        source::cell(&assignment.replacement),
                    )
                })
                .collect();
            if reverse {
                values.reverse();
            }
            (*column, values)
        })
        .collect();
    if reverse {
        groups.reverse();
    }
    groups
}

fn prove(case: &Case, plan: &NativeEditPlan, reverse: bool) -> Result<(), EditError> {
    let requests = original(case, reverse);
    let groups: Vec<_> = requests
        .iter()
        .map(|(column, values)| FixedColumnUpdates::new(case.table, column, values))
        .collect();
    proof::prove(
        plan,
        CatalogScope::new(281),
        case.table,
        &groups,
        &mut budget(),
        &|| false,
    )
}

fn literal(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone(),
        after: case.after.clone(),
        changed_rows: case.changed,
    }
}

#[test]
#[ignore = "requires all independently qualified wider UPDATE source images"]
fn literal_wider_indexed_plans_pass_fresh_manual_proof_in_both_original_orders() {
    let cases = cases();
    assert_eq!(cases.len(), 48);
    for case in cases {
        for reverse in [false, true] {
            prove(&case, &literal(&case), reverse).unwrap_or_else(|error| {
                panic!(
                    "{} {} reverse={reverse}: {error:?}",
                    case.table, case.recipe
                )
            });
        }
        let empty = NativeEditPlan {
            before: case.before.clone(),
            after: case.before.clone(),
            changed_rows: 0,
        };
        let groups: Vec<_> = case
            .groups
            .iter()
            .map(|(column, _)| FixedColumnUpdates::new(case.table, column, &[]))
            .collect();
        proof::prove(
            &empty,
            CatalogScope::new(281),
            case.table,
            &groups,
            &mut budget(),
            &|| false,
        )
        .unwrap();
    }
}

#[test]
#[ignore = "requires independently qualified wider UPDATE source images"]
fn manual_wider_proof_rejects_history_row_mask_map_padding_and_original_tail_changes() {
    for case in cases()
        .iter()
        .filter(|case| matches!(case.recipe, "joint-marker" | "primary-swap-extremes"))
    {
        for offset in source::forged_offsets(case) {
            let mut forged = literal(case);
            forged.after[offset] ^= 1;
            assert_eq!(prove(case, &forged, false), Err(EditError::Preservation));
        }
        let mut count = literal(case);
        count.changed_rows += 1;
        assert_eq!(prove(case, &count, false), Err(EditError::Preservation));

        // The original inactive byte is still readable and protected; an old
        // literal AFTER must not silently restore the earlier original tail.
        let mut before = literal(case);
        before.before[case.layout.roots[1] as usize * 4096 + 4095] ^= 1;
        assert_eq!(prove(case, &before, false), Err(EditError::Preservation));
    }
}

#[test]
#[ignore = "requires independently qualified wider UPDATE source originals"]
fn direct_wider_proof_refuses_stale_original_expected_carriers() {
    for case in cases()
        .iter()
        .filter(|case| matches!(case.recipe, "joint-marker" | "primary-swap-extremes"))
    {
        let mut requests = original(case, false);
        let first = &requests[0].1[0];
        let replacement = source::cell(first.value());
        let row = first.row();
        requests[0].1[0] = FixedScalarUpdate::new(row, Value::Null, replacement);
        let groups: Vec<_> = requests
            .iter()
            .map(|(column, values)| FixedColumnUpdates::new(case.table, column, values))
            .collect();
        assert_eq!(
            proof::prove(
                &literal(case),
                CatalogScope::new(281),
                case.table,
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Conflict)
        );
    }
}
