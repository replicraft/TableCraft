//! Child of normal_ordinary::indexed; independent SOURCE outcomes only.
use super::proof;
use crate::{
    EditError, FixedColumnUpdates, FixedScalarUpdate, IndexConstraintViolation, InspectError,
    NativeEditPlan, NativeFixedIntentGroup, NativeImage, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, WriteBlocker,
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed_targets::{
    self as model, Case, Recipe, Refusal,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 1_000_000_000)
}

fn scope() -> CatalogScope {
    CatalogScope::new(119)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn cell(value: Option<i32>) -> Value {
    value.map(Value::Int32).unwrap_or(Value::Null)
}

fn updates(case: &Case) -> Vec<FixedScalarUpdate> {
    case.updates
        .iter()
        .map(|update| {
            FixedScalarUpdate::new(update.row, cell(update.expected), cell(update.replacement))
        })
        .collect()
}

fn groups(case: &Case) -> Vec<NativeFixedIntentGroup> {
    vec![NativeFixedIntentGroup::new("Id", updates(case), &mut budget(), &|| false).unwrap()]
}

fn literal_plan(case: &Case) -> NativeEditPlan {
    NativeEditPlan {
        before: case.before.clone().into(),
        after: case
            .expected
            .as_ref()
            .expect("positive SOURCE outcome")
            .clone(),
        changed_rows: case.changed_rows,
    }
}

fn prove(case: &Case, plan: &NativeEditPlan) -> Result<(), EditError> {
    let updates = updates(case);
    let original = [FixedColumnUpdates::new("Items", "Id", &updates)];
    proof::prove(plan, scope(), "Items", &original, &mut budget(), &|| false)
}

fn row_bounds(bytes: &[u8], row: usize) -> (usize, usize) {
    let base = 8 * 4096;
    let word = base + 14 + row * 2;
    let start = u16::from_le_bytes(bytes[word..word + 2].try_into().unwrap());
    assert_eq!(start & 0xe000, 0);
    let end = if row == 0 {
        4096
    } else {
        usize::from(u16::from_le_bytes(
            bytes[word - 2..word].try_into().unwrap(),
        ))
    };
    (base + usize::from(start), base + end)
}

#[test]
fn ordinary_indexed_public_plans_match_twenty_four_complete_source_outcomes() {
    let mut outcomes = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before, "{}", recipe.id());
            assert_eq!(plan.after_bytes(), expected, "{}", recipe.id());
            assert_eq!(plan.changed_rows(), case.changed_rows, "{}", recipe.id());
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(&wire[..8], b"TCNI0004");
            let decoded = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            let original = decoded.fixed().unwrap().groups();
            assert_eq!(original.len(), 1);
            assert_eq!(original[0].column(), "Id");
            assert_eq!(original[0].updates().len(), case.updates.len());
            for (actual, supplied) in original[0].updates().iter().zip(&case.updates) {
                assert_eq!(actual.row(), supplied.row);
                assert_eq!(actual.expected(), &cell(supplied.expected));
                assert_eq!(actual.value(), &cell(supplied.replacement));
            }
            outcomes += 1;
        }
    }
    assert_eq!(outcomes, 24);
}

#[test]
fn ordinary_indexed_refusals_keep_native_constraint_conflict_and_generated_errors() {
    let mut refusals = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(refusal) = case.refusal else {
                continue;
            };
            assert!(case.expected.is_none());
            let error = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("original SOURCE refusal");
            // These mappings follow the actual field/stale/final-key check
            // owners; runtime discrepancies must be retained and diagnosed.
            match refusal {
                Refusal::DuplicateKey => assert_eq!(
                    error,
                    RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::DuplicateKey
                    )))
                ),
                Refusal::Conflict => {
                    assert_eq!(error, RecoverableEditError::Edit(EditError::Conflict))
                }
                Refusal::GeneratedColumn => assert_eq!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column))
                ),
                Refusal::RequiredIndexKey => assert_eq!(
                    error,
                    RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::NullKey
                    )))
                ),
            }
            refusals += 1;
        }
    }
    assert_eq!(refusals, 8);
}

#[test]
fn ordinary_indexed_proof_accepts_all_literal_outcomes_without_producer_calls() {
    let mut outcomes = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            if case.expected.is_none() {
                continue;
            }
            let plan = literal_plan(&case);
            prove(&case, &plan).unwrap();
            outcomes += 1;
        }
    }
    assert_eq!(outcomes, 24);
}

#[test]
fn ordinary_indexed_proof_rejects_index_masks_history_tails_and_wrong_counts() {
    for header in [5, 6] {
        for recipe in [
            Recipe::PromoteNull,
            Recipe::ClearValue,
            Recipe::Aliases,
            Recipe::ManualTree,
            Recipe::PrefixLoss,
        ] {
            let case = model::source(header, recipe);
            let row = case.updates[0].row;
            let (start, end) = row_bounds(&case.before, row);
            let offsets = [
                9 * 4096 + 480,
                end - 1,
                start + 6,
                7 * 4096 + 67,
                7 * 4096 + 20,
                9 * 4096 + 4095,
            ];
            for offset in offsets {
                let mut forged = literal_plan(&case);
                forged.after[offset] ^= 1;
                assert_eq!(
                    prove(&case, &forged),
                    Err(EditError::Preservation),
                    "{} at{offset}",
                    recipe.id()
                );
            }
            for wrong_count in [0, case.changed_rows + 1] {
                let mut forged = literal_plan(&case);
                forged.changed_rows = wrong_count;
                assert_eq!(prove(&case, &forged), Err(EditError::Preservation));
            }
        }
    }
}

#[test]
fn ordinary_indexed_proof_preserves_inactive_null_payload_and_original_intent() {
    for header in [5, 6] {
        for recipe in [Recipe::ClearValue, Recipe::UniqueAllNull, Recipe::NullNoop] {
            let case = model::source(header, recipe);
            let row = if recipe == Recipe::NullNoop { 1 } else { 0 };
            let (start, end) = row_bounds(&case.before, row);
            let mut forged = literal_plan(&case);
            assert_eq!(forged.after[end - 1] & 1, 0);
            forged.after[start + 2] ^= 0x40;
            assert_eq!(prove(&case, &forged), Err(EditError::Preservation));
        }

        // This is another individually valid, same-width request. Its literal
        // outcome cannot certify the separately supplied original Id7 intent.
        let mut case = model::source(header, Recipe::ManualSeven);
        let plan = literal_plan(&case);
        case.updates[0].replacement = Some(8);
        assert_eq!(prove(&case, &plan), Err(EditError::Preservation));
    }
}
