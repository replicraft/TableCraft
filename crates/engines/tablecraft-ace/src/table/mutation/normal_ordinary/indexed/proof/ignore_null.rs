//! Literal independent plans enter the actual private omission proof directly.
use super::prove;
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexConstraintViolation, InspectError,
    NativeEditPlan, WriteBlocker,
};
use sha2::{Digest, Sha256};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::genuine_ignore_null_update::{self as source, Recipe};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

#[test]
#[ignore = "requires held IgnoreNULL original/AFTER images"]
fn literal_omission_population_history_and_preservation_are_independently_proved() {
    let cases = source::load_all(|raw, expected| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    });
    let mut counts = [0; 4];
    for case in cases {
        let updates: Vec<_> = case
            .updates()
            .into_iter()
            .map(|(row, before, after)| FixedScalarUpdate::new(row, before, after))
            .collect();
        let groups = [FixedColumnUpdates::new("Single", "Left", &updates)];
        let scope = CatalogScope::new(264);
        let plan = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: case.changed(),
        };
        prove(&plan, scope, "Single", &groups, &mut budget(), &|| false).unwrap();
        counts[0] += 1;
        for offset in case.forged_offsets() {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            let forged = NativeEditPlan {
                before: case.before.clone().into(),
                after,
                changed_rows: case.changed(),
            };
            assert_eq!(
                prove(&forged, scope, "Single", &groups, &mut budget(), &|| false),
                Err(EditError::Preservation)
            );
            counts[1] += 1;
        }
        let wrong_count = NativeEditPlan {
            before: case.before.clone().into(),
            after: case.after.clone(),
            changed_rows: case.changed() + 1,
        };
        assert_eq!(
            prove(
                &wrong_count,
                scope,
                "Single",
                &groups,
                &mut budget(),
                &|| false
            ),
            Err(EditError::Preservation)
        );
        counts[2] += 1;
        // Replacing the two other unchanged-image groups would repeat exactly
        // the represented no-op's stale input, plan and assertion.
        if !matches!(case.recipe, Recipe::OmittedNoop | Recipe::Empty) {
            let stale = [FixedScalarUpdate::new(
                2,
                Value::Int32(10),
                Value::Int32(30),
            )];
            let groups = [FixedColumnUpdates::new("Single", "Left", &stale)];
            assert_eq!(
                prove(&plan, scope, "Single", &groups, &mut budget(), &|| false),
                Err(EditError::Conflict)
            );
            counts[3] += 1;
        }
    }
    assert_eq!(counts, [12, 324, 12, 8]);
}

#[test]
#[ignore = "requires held genuine Required/Unique and Single originals"]
fn independent_proof_refuses_required_unique_and_ambiguous_family_requests() {
    let cases = source::load_all(|raw, expected| {
        let actual: String = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(actual, expected);
    });
    let mut refusals = 0;
    for case in cases
        .into_iter()
        .filter(|case| case.recipe == Recipe::NullNew)
    {
        for refusal in source::REFUSALS {
            let updates: Vec<_> = refusal
                .updates()
                .into_iter()
                .map(|(row, before, after)| FixedScalarUpdate::new(row, before, after))
                .collect();
            let groups = [FixedColumnUpdates::new(refusal.table(), "Left", &updates)];
            // This deliberately unqualified AFTER does not pretend to be a new
            // expected image. Original requests must refuse before comparison.
            let plan = NativeEditPlan {
                before: case.before.clone().into(),
                after: case.before.clone(),
                changed_rows: 0,
            };
            let expected = match refusal {
                source::Refusal::Required => EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::NullKey,
                )),
                source::Refusal::Unique => EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey,
                )),
                _ => EditError::Unsupported(WriteBlocker::IndexedColumn),
            };
            assert_eq!(
                prove(
                    &plan,
                    CatalogScope::new(268),
                    refusal.table(),
                    &groups,
                    &mut budget(),
                    &|| false,
                ),
                Err(expected)
            );
            refusals += 1;
        }
    }
    assert_eq!(refusals, 12);
}
