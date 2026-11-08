//! Direct proof observations; no stage, planner, journal or writer builds them.
use super::proof;
use crate::{EditError, Int32Update, NativeEditPlan, WriteBlocker};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_mixed_before_change::{
    self as model, Case, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn updates(case: &Case) -> Vec<Int32Update> {
    case.updates
        .iter()
        .map(|update| Int32Update::new(update.ordinal, update.expected, update.replacement))
        .collect()
}

fn observation(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone().into(),
        after: case.expected.as_ref().unwrap().clone(),
        changed_rows: case.changed_rows,
    }
}

fn prove(plan: &NativeEditPlan, column: &str, requests: &[Int32Update]) -> Result<(), EditError> {
    proof::prove(
        plan,
        CatalogScope::new(104),
        "Items",
        column,
        requests,
        &mut budget(),
        &|| false,
    )
}

fn record_start(bytes: &[u8], ordinal: usize) -> usize {
    let word = 8 * 4096 + 14 + 2 * ordinal;
    let offset = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    assert_eq!(offset & 0xe000, 0);
    8 * 4096 + usize::from(offset)
}

#[test]
fn independent_mixed_proof_accepts_all_sixteen_literal_images_without_staging() {
    let mut count = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            if case.expected.is_none() {
                continue;
            }
            let plan = observation(&case);
            prove(&plan, case.column, &updates(&case)).unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.expected.as_ref().unwrap());
            if matches!(
                recipe,
                Recipe::FixedRestore | Recipe::CounterRestore | Recipe::CounterEmpty
            ) {
                assert_eq!(plan.after_bytes(), plan.before_bytes());
                assert_eq!(plan.changed_rows(), 0);
            }
            count += 1;
        }
    }
    assert_eq!(count, 16);
}

#[test]
fn independent_mixed_proof_rejects_wrong_count_length_and_every_retained_byte_class() {
    for header in [5, 6] {
        for recipe in [
            Recipe::FixedSequential,
            Recipe::CounterPrimary,
            Recipe::CounterRestore,
            Recipe::CounterEmpty,
        ] {
            let case = model::source(header, recipe);
            let requests = updates(&case);
            let start = record_start(&case.before, 0);
            let mut offsets = vec![
                200,                     // unrelated header byte
                7 * 4096 + 100,          // retained definition descriptor
                8 * 4096 + 2,            // DATA free count
                8 * 4096 + 14,           // logical root directory word
                start + 2 + case.offset, // selected physical target payload
                9 * 4096 - 1,            // row0 presence mask
            ];
            if recipe == Recipe::FixedSequential {
                // Actual unchanged SingleValue at fixed offset23: its signed
                // zero bits are preservation data, not a numeric equality.
                offsets.push(start + 2 + 23);
            } else {
                offsets.push(start + 2); // unchanged readonly counter Id
                offsets.extend([9 * 4096, 9 * 4096 + 480, 9 * 4096 + 700]);
            }
            for offset in offsets {
                let mut forged = observation(&case);
                forged.after[offset] ^= 1;
                assert_eq!(
                    prove(&forged, case.column, &requests),
                    Err(EditError::Preservation)
                );
            }
            let mut forged = observation(&case);
            forged.changed_rows += 1;
            assert_eq!(
                prove(&forged, case.column, &requests),
                Err(EditError::Preservation)
            );
            let mut short = observation(&case);
            short.after.pop().unwrap();
            assert_eq!(
                prove(&short, case.column, &requests),
                Err(EditError::Preservation)
            );
            let mut long = observation(&case);
            long.after.push(0);
            assert_eq!(
                prove(&long, case.column, &requests),
                Err(EditError::Preservation)
            );
        }
    }
}

#[test]
fn fresh_original_programs_and_written_requests_determine_the_expected_image() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::CounterPrimary);
        let plan = observation(&case);
        assert_eq!(
            prove(&plan, "Score", &[Int32Update::new(0, 1001, 17)]),
            Err(EditError::Conflict),
        );
        assert_eq!(
            prove(&plan, "Score", &[Int32Update::new(0, 1000, 19)]),
            Err(EditError::Preservation),
        );
        assert_eq!(prove(&plan, "Score", &[]), Err(EditError::Preservation));
        assert_eq!(
            prove(&plan, "Id", &[Int32Update::new(0, 1, 7)]),
            Err(EditError::Unsupported(WriteBlocker::Column)),
        );
        let mut other_program = observation(&case);
        // The other committed original has the same table, index and rows but
        // a different native-owned program. Reusing the old AFTER must fail.
        other_program.before = model::source(header, Recipe::CounterRestore).before.into();
        assert_eq!(
            prove(&other_program, "Score", &updates(&case)),
            Err(EditError::Preservation)
        );

        let ordinary_counter = model::source(header, Recipe::CounterUnicode);
        let mut other_operand = observation(&ordinary_counter);
        let start = record_start(&other_operand.before, 0);
        assert_eq!(
            &other_operand.before[start + 2..start + 6],
            &1_i32.to_le_bytes()
        );
        // No index exists on this source. Changing the readonly counter operand
        // changes the independently replayed result; no stale frame may survive.
        let mut altered_before = other_operand.before.to_vec();
        altered_before[start + 2..start + 6].copy_from_slice(&2_i32.to_le_bytes());
        other_operand.before = altered_before.into();
        assert_eq!(
            prove(&other_operand, "Score", &updates(&ordinary_counter)),
            Err(EditError::Preservation)
        );

        let restore = model::source(header, Recipe::CounterRestore);
        // Distinct valid user proposals can intentionally restore the SAME image.
        // This physical proof checks semantics, while bundle intent binding owns
        // the exact historical proposal. Do not mislabel a valid replay forgery.
        prove(
            &observation(&restore),
            "Score",
            &[Int32Update::new(0, 1000, 19)],
        )
        .unwrap();
    }
}

#[test]
fn direct_proof_refuses_generated_dormant_opaque_and_equal_assignment_contexts() {
    for header in [5, 6] {
        for recipe in [
            Recipe::DormantCounter,
            Recipe::UserCounter,
            Recipe::OpaqueOperand,
            Recipe::LateRaise,
        ] {
            let case = model::source(header, recipe);
            assert!(case.expected.is_none());
            // A fabricated unchanged observation is not an expected image for
            // a refused request. The proof must qualify BEFORE independently.
            let observed = NativeEditPlan {
                before: case.before.clone().into(),
                after: case.before.clone(),
                changed_rows: 0,
            };
            let error = prove(&observed, case.column, &updates(&case)).unwrap_err();
            match recipe {
                Recipe::OpaqueOperand => {
                    assert_eq!(error, EditError::Unsupported(WriteBlocker::ActiveContent))
                }
                Recipe::LateRaise => assert_eq!(error, EditError::Preservation),
                _ => assert_eq!(error, EditError::Unsupported(WriteBlocker::Column)),
            }
            if recipe != Recipe::LateRaise {
                assert_eq!(prove(&observed, case.column, &[]), Err(error));
            }
        }
        let case = model::source(header, Recipe::CounterPrimary);
        let observed = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.before.clone(),
            changed_rows: 0,
        };
        assert_eq!(
            prove(&observed, "Score", &[Int32Update::new(0, 1000, 1000)]),
            Err(EditError::Unsupported(WriteBlocker::EventTiming)),
        );
    }
}
