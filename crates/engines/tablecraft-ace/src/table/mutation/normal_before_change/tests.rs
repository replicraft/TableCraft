//! Direct independent-proof controls: every observation starts from a literal
//! source/expected pair, never from stage patches or a generated candidate.
use super::proof;
use crate::{EditError, Int32Update, NativeEditPlan};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_before_change::{Recipe, source};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn prove(plan: &NativeEditPlan, requests: &[Int32Update]) -> Result<(), EditError> {
    proof::prove(
        plan,
        CatalogScope::new(95),
        "Items",
        "Score",
        requests,
        &mut budget(),
        &|| false,
    )
}

#[test]
fn independent_proof_rejects_payload_sibling_mask_slot_metadata_and_count_forgery() {
    for header in [5, 6] {
        let case = source(header, Recipe::Sequential);
        let correct = NativeEditPlan {
            before: case.before.clone(),
            after: case.expected.as_ref().unwrap().clone(),
            changed_rows: case.changed_rows,
        };
        let requests = [Int32Update::new(0, 10, 17)];
        prove(&correct, &requests).unwrap();
        let first_word = 8 * 4096 + 14;
        let first_start = 8 * 4096
            + usize::from(
                u16::from_le_bytes(case.before[first_word..first_word + 2].try_into().unwrap())
                    & 0x1fff,
            );
        let second_word = first_word + 2;
        let second_start = 8 * 4096
            + usize::from(
                u16::from_le_bytes(
                    case.before[second_word..second_word + 2]
                        .try_into()
                        .unwrap(),
                ) & 0x1fff,
            );
        assert_eq!(&case.before[first_start..first_start + 2], &[2, 0]);
        assert_eq!(&case.before[second_start..second_start + 2], &[2, 0]);
        for offset in [
            first_start + 6,  // Selected Score payload differs from reconstructed28.
            first_start + 2,  // Same root's unrelated Id.
            second_start + 6, // Unselected row's Score.
            first_start + 10, // Presence mask, never writable for this profile.
            first_word,       // Physical root directory word.
            7 * 4096 + 16,    // TD row-count metadata.
            200,              // Unrelated retained header bytes.
        ] {
            let mut forged = NativeEditPlan {
                before: correct.before_bytes().to_vec(),
                after: correct.after_bytes().to_vec(),
                changed_rows: correct.changed_rows(),
            };
            forged.after[offset] ^= 1;
            assert_eq!(prove(&forged, &requests), Err(EditError::Preservation));
        }
        let wrong_count = NativeEditPlan {
            before: correct.before_bytes().to_vec(),
            after: correct.after_bytes().to_vec(),
            changed_rows: 0,
        };
        assert_eq!(prove(&wrong_count, &requests), Err(EditError::Preservation));
        assert_eq!(
            prove(&correct, &[Int32Update::new(0, 10, 19)]),
            Err(EditError::Preservation),
        );
        assert_eq!(
            prove(&correct, &[Int32Update::new(0, 11, 17)]),
            Err(EditError::Conflict),
        );
    }
}

#[test]
fn restore_original_keeps_distinct_valid_intents_without_inventing_historical_authority() {
    for header in [5, 6] {
        let case = source(header, Recipe::RestoreOriginal);
        let observation = NativeEditPlan {
            before: case.before.clone(),
            after: case.expected.as_ref().unwrap().clone(),
            changed_rows: 0,
        };
        assert_eq!(observation.before_bytes(), observation.after_bytes());
        prove(&observation, &[Int32Update::new(0, 10, 17)]).unwrap();
        // A separately supplied and validated19 is also restored by Old10.
        // Pair binding must retain the exact encoded request; fullimage equality
        // alone cannot distinguish two legitimate original requests.
        prove(&observation, &[Int32Update::new(0, 10, 19)]).unwrap();
        assert_eq!(
            prove(&observation, &[Int32Update::new(0, 10, 10)]),
            Err(EditError::Unsupported(crate::WriteBlocker::EventTiming)),
        );
    }
}
