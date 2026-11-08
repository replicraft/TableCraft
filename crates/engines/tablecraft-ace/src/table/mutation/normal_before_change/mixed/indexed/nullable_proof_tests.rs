//! Literal nullable-key proofs without invoking the production planner.
use super::prove;
use crate::{EditError, Int32Update, NativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_nullable_key_before_change as model;

const SCOPE: CatalogScope = CatalogScope::new(111);

fn audit(plan: &NativeEditPlan, case: &model::Case) -> Result<(), EditError> {
    prove(
        plan,
        SCOPE,
        "Items",
        "Score",
        &[Int32Update::new(
            case.update.ordinal,
            case.update.expected,
            case.update.replacement,
        )],
        &mut ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000),
        &|| false,
    )
}

#[test]
fn nullable_key_proof_accepts_twelve_literal_outcomes_without_planning() {
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let plan = NativeEditPlan {
                before: case.before.clone().into(),
                after: case.expected.clone(),
                changed_rows: 1,
            };
            audit(&plan, &case).unwrap();
        }
    }
}

#[test]
fn nullable_key_proof_rejects_inactive_payload_and_index_complement_changes() {
    for recipe in model::RECIPES {
        let case = model::source(6, recipe);
        let ordinal = case.null_rows[0];
        let word = 8 * 4096 + 14 + 2 * ordinal;
        let start = 8 * 4096
            + usize::from(u16::from_le_bytes(
                case.before[word..word + 2].try_into().unwrap(),
            ));
        let mut offsets = vec![
            200,            // Unrelated database header.
            3 * 4096 + 2,   // Retained allocation metadata.
            9 * 4096 + 480, // Preserved PrimaryId entry.
            start + 6,      // Inactive NULL Score payload.
        ];
        for &page in &case.owned {
            offsets.extend([
                page as usize * 4096 + 24,  // Original prefix width.
                page as usize * 4096 + 700, // Unused retained leaf/node bytes.
            ]);
        }

        for offset in offsets {
            let mut after = case.expected.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone().into(),
                after,
                changed_rows: 1,
            };
            assert!(audit(&forged, &case).is_err(), "{recipe:?} byte {offset}");
        }

        let wrong_count = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.expected.clone(),
            changed_rows: 0,
        };
        assert!(audit(&wrong_count, &case).is_err());
    }
}
