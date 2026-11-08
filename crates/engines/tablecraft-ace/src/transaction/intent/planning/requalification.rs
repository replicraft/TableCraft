//! Fresh-source reconstruction retains the reviewed original operation exactly.
mod fixed;
mod growth;

use super::RecoverableNativeEditPlan;
use crate::{
    EditError, FixedScalarUpdate, InspectError, Int32Update, NativeFittedIntentGroup,
    NativeFittedUpdates, NativeFixedIntentGroup, NativeImage, NativeRecoveryQualificationError,
    NativeUpdateProfile, SlotScalarUpdate,
};
use std::io::Cursor;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::fixtures::access::{
    canonical_before_change as events, canonical_ordinary_content as ordinary,
    canonical_ordinary_fitted as fitted, mutation_fixture,
};

const TEST_BYTES: u64 = 1_000_000_000;
const TEST_ROWS: u64 = 1_000_000;
const TEST_STEPS: u64 = 1_000_000_000;
const SCOPE: CatalogScope = CatalogScope::new(243);

fn budget() -> ResourceBudget {
    ResourceBudget::new(TEST_BYTES, TEST_ROWS, TEST_STEPS)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn check_requalification(
    plan: RecoverableNativeEditPlan,
    before: &[u8],
    expected: &[u8],
    changed: usize,
    profile: NativeUpdateProfile,
) {
    let original = plan.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(plan.before_bytes(), before);
    assert_eq!(plan.after_bytes(), expected);
    assert_eq!(plan.changed_rows(), changed);
    assert_eq!(plan.intent().profile(), profile);

    let fresh = plan
        .requalify(image(before), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fresh.before_bytes(), before);
    assert_eq!(fresh.after_bytes(), expected);
    assert_eq!(fresh.changed_rows(), changed);
    assert_eq!(fresh.intent().profile(), profile);
    assert_eq!(
        fresh.intent().encode(&mut budget(), &|| false).unwrap(),
        original
    );
}

fn jet4(before: &[u8], replacement: i32) -> RecoverableNativeEditPlan {
    image(before)
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, replacement)],
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn fitted_groups(case: &fitted::Case) -> Vec<NativeFittedIntentGroup> {
    case.groups
        .iter()
        .map(|group| {
            let updates = if matches!(group.number, 1 | 2) {
                NativeFittedUpdates::Variable(
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            SlotScalarUpdate::new(
                                update.row,
                                update.expected.clone(),
                                update.replacement.clone(),
                            )
                        })
                        .collect(),
                )
            } else {
                NativeFittedUpdates::Fixed(
                    group
                        .updates
                        .iter()
                        .map(|update| {
                            FixedScalarUpdate::new(
                                update.row,
                                update.expected.clone(),
                                update.replacement.clone(),
                            )
                        })
                        .collect(),
                )
            };
            NativeFittedIntentGroup::new(group.column, updates, &mut budget(), &|| false).unwrap()
        })
        .collect()
}

#[test]
fn jet4_and_ordinary_fixed_reproduce_independent_complete_images_and_original_intents() {
    let before = mutation_fixture::database();
    let expected = mutation_fixture::with_rows(&[(1, 77), (2, 20), (3, 30)]);
    check_requalification(
        jet4(&before, 77),
        &before,
        &expected,
        1,
        NativeUpdateProfile::Jet4Int32,
    );

    for header in [5, 6] {
        let mut source = ordinary::source(ordinary::Case::Extrema);
        source.bytes[20] = header;
        let table = &source.tables[0];
        let Value::Int32(before_value) = table.rows[0][0] else {
            panic!("raw fixture's first field must remain Int32");
        };
        let mut expected = source.bytes.clone();
        let base = table.data[0] as usize * 4096;
        let slot = u16::from_le_bytes(expected[base + 14..base + 16].try_into().unwrap());
        assert_eq!(slot & 0xe000, 0);
        let start = base + usize::from(slot);
        expected[start + 2..start + 6].copy_from_slice(&73_i32.to_le_bytes());
        let group = NativeFixedIntentGroup::new(
            &table.columns[0],
            vec![FixedScalarUpdate::new(
                0,
                Value::Int32(before_value),
                Value::Int32(73),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = image(&source.bytes)
            .plan_recoverable_ordinary_fixed_updates(
                &table.name,
                vec![group],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        check_requalification(
            plan,
            &source.bytes,
            &expected,
            1,
            NativeUpdateProfile::Ace16OrdinaryFixed,
        );
    }
}

#[test]
fn fitting_and_positive_event_profiles_reproduce_raw_oracles_including_effective_noops() {
    for header in [5, 6] {
        for recipe in [fitted::Recipe::RawUnicode, fitted::Recipe::ExactNoops] {
            let case = fitted::source(header, recipe);
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    fitted_groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            check_requalification(
                plan,
                &case.before,
                case.expected.as_ref().unwrap(),
                case.changed,
                NativeUpdateProfile::Ace16OrdinaryFitted,
            );
        }
        for recipe in [events::Recipe::Sequential, events::Recipe::RestoreOriginal] {
            let case = events::source(header, recipe);
            let updates = case
                .updates
                .iter()
                .map(|update| Int32Update::new(update.ordinal, update.expected, update.replacement))
                .collect();
            let plan = image(&case.before)
                .plan_recoverable_int32_updates("Items", "Score", updates, &mut budget(), &|| false)
                .unwrap();
            check_requalification(
                plan,
                &case.before,
                case.expected.as_ref().unwrap(),
                case.changed_rows,
                NativeUpdateProfile::Ace16BeforeChangeInt32,
            );
        }
    }
}

#[test]
fn complete_fresh_before_binding_refuses_same_length_changed_data_and_protected_tail_on_noops() {
    let original = mutation_fixture::database();
    let changed_data = mutation_fixture::with_rows(&[(1, 10), (2, 88), (3, 30)]);
    let mut changed_tail = original.clone();
    *changed_tail.last_mut().unwrap() ^= 1;
    for (case, changed) in [
        ("unselected record", changed_data),
        ("protected tail", changed_tail),
    ] {
        assert_eq!(changed.len(), original.len());
        let reviewed = jet4(&original, 10);
        assert_eq!(reviewed.changed_rows(), 0);
        assert_eq!(reviewed.before_bytes(), reviewed.after_bytes());
        let error = reviewed
            .requalify(image(&changed), &mut budget(), &|| false)
            .err();
        assert!(
            matches!(
                &error,
                Some(NativeRecoveryQualificationError::ImageMismatch)
            ),
            "{case}: {error:?}"
        );
    }
}

#[test]
fn complete_rebuilt_after_must_match_review_even_when_counts_and_before_agree() {
    let before = mutation_fixture::database();
    let mut reviewed = jet4(&before, 77);
    let different = jet4(&before, 88);
    assert_eq!(reviewed.before_bytes(), different.before_bytes());
    assert_eq!(reviewed.changed_rows(), different.changed_rows());
    assert_ne!(reviewed.after_bytes(), different.after_bytes());
    // Layer-local adversarial type glue: both constituent plans are legitimate;
    // only their reviewed-plan/original-intent pairing is deliberately wrong.
    reviewed.plan = different.plan;
    assert!(matches!(
        reviewed.requalify(image(&before), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
}

#[test]
fn fresh_requalification_keeps_cancellation_and_resource_failures_typed() {
    let before = mutation_fixture::database();
    assert!(matches!(
        jet4(&before, 77).requalify(image(&before), &mut budget(), &|| true),
        Err(NativeRecoveryQualificationError::Inspect(
            InspectError::Cancelled
        ))
    ));
    let zero_steps = jet4(&before, 77)
        .requalify(
            image(&before),
            &mut ResourceBudget::new(TEST_BYTES, TEST_ROWS, 0),
            &|| false,
        )
        .err();
    assert!(
        matches!(zero_steps, Some(NativeRecoveryQualificationError::Inspect(InspectError::Limit(error))) if error.resource == Resource::Steps)
    );
    let zero_bytes = jet4(&before, 77)
        .requalify(
            image(&before),
            &mut ResourceBudget::new(0, TEST_ROWS, TEST_STEPS),
            &|| false,
        )
        .err();
    assert!(
        matches!(zero_bytes, Some(NativeRecoveryQualificationError::Edit(EditError::Inspect(InspectError::Limit(error)))) if error.resource == Resource::Bytes)
    );
}
