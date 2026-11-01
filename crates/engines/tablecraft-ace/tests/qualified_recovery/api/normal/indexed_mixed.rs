//! Child of qualified_recovery::api::normal; existing storage and framing only.
use super::{
    Storage, budget, intent, model as journals, planning_inspection, recovery_inspection, scope,
};
use journals::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, Int32Update, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateProfile, RecoverableEditError,
    RecoverableNativeEditPlan, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::canonical_indexed_mixed_before_change::{
    self as model, Case, Recipe,
};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(case: &Case) -> Vec<Int32Update> {
    case.updates
        .iter()
        .map(|update| Int32Update::new(update.ordinal, update.expected, update.replacement))
        .collect()
}

fn record(
    case: &Case,
    work: &mut ResourceBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
    image(&case.before).plan_recoverable_int32_updates(
        "Items",
        "Score",
        requests(case),
        work,
        cancel,
    )
}

#[test]
fn indexed_mixed_commits_twelve_literal_images_and_keeps_exact_original_tcni2_requests() {
    let mut images = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let original = requests(&case);
            let pointer = original.as_ptr();
            let plan = image(&case.before)
                .plan_recoverable_int32_updates("Items", "Score", original, &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.changed_rows(), case.changed_rows);
            assert_eq!(plan.after_bytes().len(), case.before.len());
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16BeforeChangeInt32
            );
            assert_eq!(
                plan.intent()
                    .int32()
                    .expect("Int32 intent")
                    .updates()
                    .as_ptr(),
                pointer
            );
            assert_eq!(
                plan.intent().int32().expect("Int32 intent").updates().len(),
                case.updates.len()
            );
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                intent(&case.before, requests(&case))
                    .encode(&mut budget(), &|| false)
                    .unwrap()
            );
            for (actual, supplied) in plan
                .intent()
                .int32()
                .expect("Int32 intent")
                .updates()
                .iter()
                .zip(&case.updates)
            {
                assert_eq!(
                    (actual.row(), actual.expected(), actual.value()),
                    (supplied.ordinal, supplied.expected, supplied.replacement)
                );
            }
            if case.primary {
                assert_eq!(
                    &plan.after_bytes()[9 * 4096..10 * 4096],
                    &case.before[9 * 4096..10 * 4096]
                );
            }
            if recipe == Recipe::CounterTreeRetainedRestore {
                assert_eq!(plan.after_bytes(), case.before);
                assert_eq!(plan.changed_rows(), 0);
                assert_eq!(
                    plan.intent().int32().expect("Int32 intent").updates()[0].value(),
                    2599
                );
            }
            if recipe == Recipe::CounterUniqueSwap {
                let mut reversed = requests(&case);
                reversed.reverse();
                let reverse = image(&case.before)
                    .plan_recoverable_int32_updates(
                        "Items",
                        "Score",
                        reversed,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(reverse.after_bytes(), expected);
                assert_eq!(reverse.changed_rows(), 2);
                assert_eq!(
                    reverse
                        .intent()
                        .int32()
                        .expect("Int32 intent")
                        .updates()
                        .iter()
                        .map(Int32Update::row)
                        .collect::<Vec<_>>(),
                    [2, 0]
                );
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                prepared.commit(&mut budget(), &|| false).unwrap(),
                case.changed_rows
            );
            assert_eq!(target.bytes.get_ref(), expected);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, expected, Marker::Committed)
            );
            images += 1;
        }
    }
    assert_eq!(images, 12);
}

#[test]
fn indexed_mixed_unique_conflicts_stale_requests_and_generated_targets_refuse_atomically() {
    for header in [5, 6] {
        let conflict = model::source(header, Recipe::CounterUniqueConflict);
        assert!(conflict.expected.is_none());
        assert!(matches!(
            record(&conflict, &mut budget(), &|| false),
            Err(RecoverableEditError::Edit(EditError::Inspect(
                InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey)
            )))
        ));
        let case = model::source(header, Recipe::CounterLeafPrefixLoss);
        for updates in [
            vec![Int32Update::new(0, 1000, 1000)],
            vec![
                Int32Update::new(0, 1000, 2599),
                Int32Update::new(1, 2000, 2000),
            ],
        ] {
            assert!(matches!(
                image(&case.before).plan_recoverable_int32_updates(
                    "Items",
                    "Score",
                    updates,
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::EventTiming
                )))
            ));
        }
        assert!(matches!(
            image(&case.before).plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![Int32Update::new(0, 1001, 17)],
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
        assert!(matches!(
            image(&case.before).plan_recoverable_int32_updates(
                "Items",
                "Id",
                vec![Int32Update::new(0, 1, 7)],
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::Column
            )))
        ));
        // Empty is not a supplied equal assignment. It still qualifies all
        // declarations while preserving the entire retained index topology.
        let empty = image(&case.before)
            .plan_recoverable_int32_updates("Items", "Score", Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.changed_rows(), 0);
        assert!(
            empty
                .intent()
                .int32()
                .expect("Int32 intent")
                .updates()
                .is_empty()
        );
    }
}

#[test]
fn indexed_mixed_planning_exact_pools_and_final_cancel_precede_plan_publication() {
    for recipe in [
        Recipe::ManualAscending,
        Recipe::CounterCompositeTie,
        Recipe::CounterTreePrefixLoss,
    ] {
        let case = model::source(6, recipe);
        let mut measured = budget();
        let initial = POOLS.map(|resource| measured.remaining(resource));
        let polls = Cell::new(0);
        drop(
            record(&case, &mut measured, &|| {
                polls.set(polls.get() + 1);
                false
            })
            .unwrap(),
        );
        let used = std::array::from_fn::<_, 3, _>(|index| {
            initial[index] - measured.remaining(POOLS[index])
        });
        assert!(used.iter().all(|count| *count > 0));
        record(
            &case,
            &mut ResourceBudget::new(used[0], used[1], used[2]),
            &|| false,
        )
        .unwrap();
        for index in 0..3 {
            let mut limits = used;
            limits[index] -= 1;
            let error = record(
                &case,
                &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
                &|| false,
            )
            .err()
            .unwrap();
            assert!(
                matches!(planning_inspection(error), InspectError::Limit(error) if error.resource == POOLS[index])
            );
        }
        let terminal = polls.get();
        polls.set(0);
        let error = record(&case, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        })
        .err()
        .unwrap();
        assert_eq!(planning_inspection(error), InspectError::Cancelled);
        assert_eq!(
            planning_inspection(record(&case, &mut budget(), &|| true).err().unwrap()),
            InspectError::Cancelled
        );
    }
}

#[test]
fn indexed_mixed_recovery_requalifies_twelve_images_and_confirms_without_writes() {
    let mut images = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(after) = &case.expected else {
                continue;
            };
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mut journal = Storage::new(journals::journal(&case.before, after, marker));
                let saved = journal.bytes.get_ref().clone();
                let mut target = Storage::new(after.clone());
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent(&case.before, requests(&case)),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        qualified
                            .recover(&mut target, &mut budget(), &|| false)
                            .unwrap(),
                        if committed {
                            RecoveryOutcome::Committed
                        } else {
                            RecoveryOutcome::RolledBack
                        }
                    );
                    assert_eq!(
                        target.bytes.get_ref(),
                        if committed { after } else { &case.before }
                    );
                }
                if committed {
                    assert_eq!(target.writes, 0);
                }
                let writes = target.writes;
                target.fail_prefix = Some(0);
                journal.fail_prefix = Some(0);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent(&case.before, requests(&case)),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .confirm_completed(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
            images += 1;
        }
    }
    assert_eq!(images, 12);
}

#[test]
fn indexed_mixed_recovery_rejects_changed_intent_primary_score_and_sibling_images() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::CounterTreePrefixLoss);
        let after = case.expected.as_ref().unwrap();
        for variant in 0..4 {
            let mut candidate = after.clone();
            let mut original = requests(&case);
            match variant {
                0 => original = vec![Int32Update::new(2, 3000, -3101)],
                1 => candidate[9 * 4096 + 700] ^= 1,
                2 => candidate[case.root as usize * 4096 + 480] ^= 1,
                _ => candidate[200] ^= 1,
            }
            let mut journal = Storage::new(journals::journal(
                &case.before,
                &candidate,
                Marker::Committed,
            ));
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let error = qualify_native_update_recovery(
                checked,
                intent(&case.before, original),
                scope(),
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
            assert!(matches!(
                error,
                NativeRecoveryQualificationError::ImageMismatch
            ));
            assert_eq!(journal.writes, 0);
        }
        let mut target = Storage::new(case.before.clone());
        target.bytes.get_mut()[9 * 4096 + 700] ^= 1;
        let saved = target.bytes.get_ref().clone();
        let mut journal = Storage::new(Vec::new());
        assert!(matches!(
            record(&case, &mut budget(), &|| false).unwrap().prepare(
                &mut target,
                &mut journal,
                &mut budget(),
                &|| false
            ),
            Err(JournalError::TargetMismatch)
        ));
        assert_eq!(target.bytes.get_ref(), &saved);
        assert_eq!((target.writes, journal.writes), (0, 0));
        assert!(journal.bytes.get_ref().is_empty());
    }
}

#[test]
fn indexed_mixed_recovery_exact_pools_cancel_and_stale_target_precede_writes() {
    let case = model::source(6, Recipe::CounterTreePrefixLoss);
    let after = case.expected.as_ref().unwrap();
    let mut journal = Storage::new(journals::journal(&case.before, after, Marker::Pending));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let mut measured = budget();
    let initial = POOLS.map(|resource| measured.remaining(resource));
    let polls = Cell::new(0);
    drop(
        qualify_native_update_recovery(
            checked,
            intent(&case.before, requests(&case)),
            scope(),
            &mut measured,
            &|| {
                polls.set(polls.get() + 1);
                false
            },
        )
        .unwrap(),
    );
    let used =
        std::array::from_fn::<_, 3, _>(|index| initial[index] - measured.remaining(POOLS[index]));
    assert!(used.iter().all(|count| *count > 0));
    let terminal = polls.get();
    for variant in 0..5 {
        let mut limits = used;
        if variant < 3 {
            limits[variant] -= 1;
        }
        polls.set(0);
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let result = qualify_native_update_recovery(
            checked,
            intent(&case.before, requests(&case)),
            scope(),
            &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
            &|| {
                polls.set(polls.get() + 1);
                variant == 4 && polls.get() == terminal
            },
        );
        if variant == 3 {
            let mut target = Storage::new(after.clone());
            target.bytes.get_mut()[9 * 4096 + 700] ^= 1;
            let saved = target.bytes.get_ref().clone();
            assert_eq!(
                result
                    .unwrap()
                    .recover(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            );
            assert_eq!(target.writes, 0);
            assert_eq!(target.bytes.get_ref(), &saved);
        } else {
            let error = recovery_inspection(result.err().unwrap());
            if variant < 3 {
                assert!(
                    matches!(error, InspectError::Limit(error) if error.resource == POOLS[variant])
                );
            } else {
                assert_eq!(error, InspectError::Cancelled);
            }
        }
        assert_eq!(journal.writes, 0);
    }
}
