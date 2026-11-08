//! Nested child of mixed::indexed; literal observations never call a producer.
use super::{capture, prove};
use crate::{
    EditError, IndexConstraintViolation, InspectError, Int32Update, NativeEditPlan, NativeImage,
    WriteBlocker,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_indexed_mixed_before_change::{
    self as model, Case, Recipe,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(107)
}

fn requests(case: &Case) -> Vec<Int32Update> {
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

fn audit(plan: &NativeEditPlan, column: &str, updates: &[Int32Update]) -> Result<(), EditError> {
    prove(
        plan,
        scope(),
        "Items",
        column,
        updates,
        &mut budget(),
        &|| false,
    )
}

fn record_start(bytes: &[u8], ordinal: usize) -> usize {
    let at = 8 * 4096 + 14 + 2 * ordinal;
    let offset = u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    assert_eq!(offset & 0xe000, 0);
    8 * 4096 + usize::from(offset)
}

#[test]
fn indexed_mixed_independent_proof_accepts_twelve_literal_images_without_a_planner() {
    let mut images = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            if case.expected.is_none() {
                continue;
            }
            let plan = observation(&case);
            audit(&plan, "Score", &requests(&case)).unwrap();
            assert_eq!(plan.after_bytes(), case.expected.as_ref().unwrap());
            if case.primary {
                assert_eq!(
                    &plan.after_bytes()[9 * 4096..10 * 4096],
                    &case.before[9 * 4096..10 * 4096]
                );
            }
            if recipe == Recipe::CounterTreeRetainedRestore {
                assert_eq!(plan.after_bytes(), case.before);
                assert_eq!(plan.changed_rows(), 0);
            }
            images += 1;
        }
    }
    assert_eq!(images, 12);
}

#[test]
fn indexed_mixed_proof_rejects_payload_mask_maps_index_framing_and_unused_byte_forgery() {
    for recipe in [
        Recipe::ManualAscending,
        Recipe::CounterCompositeTie,
        Recipe::CounterTreePrefixLoss,
        Recipe::CounterTreeRetainedRestore,
    ] {
        let case = model::source(6, recipe);
        let start = record_start(&case.before, 0);
        let mut offsets = vec![
            200,           // unrelated header
            3 * 4096 + 2,  // retained map-container free count
            7 * 4096 + 20, // actual counter/table metadata
            8 * 4096 + 2,  // DATA page free count
            8 * 4096 + 14, // original logical row word
            start + 2,     // unchanged Id operand
            start + 6,     // original Score payload
            9 * 4096 - 1,  // original row0 mask
        ];
        for &number in &case.owned {
            for relative in [0, 2, 4, 8, 12, 16, 20, 24, 26, 27, 480, 484, 700, 4095] {
                offsets.push(number as usize * 4096 + relative);
            }
        }
        if case.primary {
            offsets.extend([9 * 4096 + 480, 9 * 4096 + 700]);
        }
        for offset in offsets {
            let mut forged = observation(&case);
            forged.after[offset] ^= 1;
            assert_eq!(
                audit(&forged, "Score", &requests(&case)),
                Err(EditError::Preservation)
            );
        }
        let mut count = observation(&case);
        count.changed_rows += 1;
        assert_eq!(
            audit(&count, "Score", &requests(&case)),
            Err(EditError::Preservation)
        );
        let mut short = observation(&case);
        short.after.pop().unwrap();
        assert_eq!(
            audit(&short, "Score", &requests(&case)),
            Err(EditError::Preservation)
        );
        let mut long = observation(&case);
        long.after.push(0);
        assert_eq!(
            audit(&long, "Score", &requests(&case)),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn indexed_mixed_proof_replays_original_requests_and_checks_unique_and_generated_rules() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::CounterCompositeTie);
        let plan = observation(&case);
        assert_eq!(
            audit(&plan, "Score", &[Int32Update::new(0, 1001, 999)]),
            Err(EditError::Conflict)
        );
        assert_eq!(
            audit(&plan, "Score", &[Int32Update::new(0, 1000, 997)]),
            Err(EditError::Preservation)
        );
        assert_eq!(audit(&plan, "Score", &[]), Err(EditError::Preservation));
        assert_eq!(
            audit(&plan, "Id", &[Int32Update::new(0, 1, 7)]),
            Err(EditError::Unsupported(WriteBlocker::Column))
        );
        let unchanged = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.before.clone(),
            changed_rows: 0,
        };
        assert_eq!(
            audit(&unchanged, "Score", &[Int32Update::new(0, 1000, 1000)]),
            Err(EditError::Unsupported(WriteBlocker::EventTiming))
        );
        audit(&unchanged, "Score", &[]).unwrap();

        let conflict = model::source(header, Recipe::CounterUniqueConflict);
        assert!(conflict.expected.is_none());
        // No fabricated conflict expected image: independent replay must reject
        // its own duplicate final key before considering this observation.
        let unchanged = NativeEditPlan {
            before: conflict.before.clone().into(),
            after: conflict.before.clone(),
            changed_rows: 0,
        };
        assert_eq!(
            audit(&unchanged, "Score", &requests(&conflict)),
            Err(EditError::Inspect(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            )))
        );

        let restore = model::source(header, Recipe::CounterTreeRetainedRestore);
        // Distinct valid proposals can share an exact restored final image;
        // physical proof is not an original-intent authenticity substitute.
        audit(
            &observation(&restore),
            "Score",
            &[Int32Update::new(0, 1000, 2499)],
        )
        .unwrap();
    }
}

#[test]
fn checked_index_targets_are_bound_to_the_exact_original_image() {
    let case = model::source(6, Recipe::CounterCompositeTie);
    let first = NativeImage::capture(
        &mut Cursor::new(&case.before),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let second = NativeImage::capture(
        &mut Cursor::new(&case.before),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let admission = capture(&first, "Items", &mut budget(), &|| false).unwrap();
    admission
        .targets
        .require_source(
            &first,
            admission.source.table.schema(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        admission.targets.require_source(
            &second,
            admission.source.table.schema(),
            &mut budget(),
            &|| false
        ),
        Err(EditError::Preservation)
    );
    // Both independent images have identical bytes and catalog IDs. Equal IDs
    // alone must not authorize a second source through the captured token.
    assert_eq!(first.as_bytes(), second.as_bytes());
}
