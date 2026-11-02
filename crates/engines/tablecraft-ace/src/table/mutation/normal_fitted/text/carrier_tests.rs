//! Frozen plans exercise the complete independent Text proof without staging.
use super::proof;
use crate::{EditError, FittedColumnUpdates, NativeEditPlan, NativeImage, SlotScalarUpdate};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_text_storage_carriers::{self as sources, Case};

const SCOPE: CatalogScope = CatalogScope::new(271);

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn updates(case: &Case) -> Vec<SlotScalarUpdate> {
    case.updates()
        .into_iter()
        .map(|(row, before, after)| SlotScalarUpdate::new(row, before, after))
        .collect()
}

fn prove(
    plan: &NativeEditPlan,
    case: &Case,
    updates: &[SlotScalarUpdate],
) -> Result<(), EditError> {
    proof::prove(
        plan,
        SCOPE,
        case.table,
        &[FittedColumnUpdates::Variable {
            column: "KeyValue",
            updates,
        }],
        &mut budget(),
        &|| false,
    )
}

#[test]
#[ignore = "requires 32 carrier source/outcome views and actual native Source qualification"]
fn literal_carrier_plans_and_unchanged_compressed_cells_prove_without_any_planner() {
    let mut proven = 0;
    for case in cases() {
        let plan = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 1,
        };
        prove(&plan, &case, &updates(&case)).unwrap();
        let unchanged = NativeEditPlan {
            before: case.before.clone(),
            after: case.before.clone(),
            changed_rows: 0,
        };
        let noop = [SlotScalarUpdate::new(
            1,
            Value::Text(case.expected.into()),
            Value::Text(case.expected.into()),
        )];
        prove(&unchanged, &case, &noop).unwrap();
        prove(&unchanged, &case, &[]).unwrap();
        proven += 3;
    }
    assert_eq!(proven, 48);
}

#[test]
#[ignore = "requires 32 source/outcome views with frozen raw storage capacity"]
fn carrier_proof_rejects_mask_modes_footer_inactive_tail_history_keys_and_counts() {
    let mut refused = 0;
    for case in cases() {
        let updates = updates(&case);
        for offset in [
            case.offset + 6,
            case.offset + 8,
            case.offset + 19,
            case.offset + 20,
            case.offset + 26,
            case.roots[1] as usize * 4096 + 480,
            case.roots[2] as usize * 4096 + 480,
            case.roots[2] as usize * 4096 + 4095,
            case.definition as usize * 4096 + 79,
        ] {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let plan = NativeEditPlan {
                before: case.before.clone(),
                after,
                changed_rows: 1,
            };
            assert_eq!(prove(&plan, &case, &updates), Err(EditError::Preservation));
            refused += 1;
        }
        let count = NativeEditPlan {
            before: case.before.clone(),
            after: case.after.clone(),
            changed_rows: 0,
        };
        assert_eq!(prove(&count, &case, &updates), Err(EditError::Preservation));
        refused += 1;
    }
    assert_eq!(refused, 160);
}

#[test]
#[ignore = "requires actual current Source and the independently qualified storage cells"]
fn collation_equivalent_but_wrong_original_carriers_refuse_before_plan_publication() {
    for case in cases() {
        let wrong = match case.carrier {
            "leading-feff" => "a",
            "compressed-latin1" => "e\u{301}",
            "compressed-ascii" => "STRASSE",
            "marked-wide" => "A",
            _ => panic!("closed original carrier recipe"),
        };
        assert_ne!(case.expected, wrong);
        let request = [SlotScalarUpdate::new(
            1,
            Value::Text(wrong.into()),
            Value::Text(case.value.into()),
        )];
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            image
                .plan_ordinary_fitted_updates(
                    case.table,
                    &[FittedColumnUpdates::Variable {
                        column: "KeyValue",
                        updates: &request
                    }],
                    &mut budget(),
                    &|| false
                )
                .err()
                .unwrap(),
            EditError::Conflict
        );
    }
}
