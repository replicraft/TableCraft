//! Child of qualified_recovery::api::normal; storage/framing stay in that owner.
use super::{Storage, budget, model as journals, planning_inspection, recovery_inspection, scope};
use journals::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, Int32Update, NativeImage, NativeInt32EditIntent,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, RecoverableNativeEditPlan, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::canonical_mixed_before_change::{self as model, Case, Recipe};

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
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
    image(&case.before).plan_recoverable_int32_updates(
        "Items",
        case.column,
        requests(case),
        work,
        cancelled,
    )
}

// Original request data only. This does not call a planner or obtain permission.
fn intent(case: &Case) -> NativeUpdateIntent {
    let original = NativeInt32EditIntent::new(
        "Items",
        case.column,
        requests(case),
        &case.before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut wire = original.encode(&mut budget(), &|| false).unwrap();
    wire[..8].copy_from_slice(b"TCNI0002");
    wire[12..14].copy_from_slice(&2_u16.to_le_bytes());
    NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap()
}

#[test]
fn mixed_normal_plans_commit_sixteen_literal_images_and_keep_original_tcni2_requests() {
    let mut images = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let updates = requests(&case);
            let pointer = updates.as_ptr();
            let plan = image(&case.before)
                .plan_recoverable_int32_updates(
                    "Items",
                    case.column,
                    updates,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.changed_rows(), case.changed_rows);
            assert_eq!(plan.after_bytes().len(), case.before.len());
            assert_eq!(
                plan.intent()
                    .int32()
                    .expect("Int32 intent")
                    .updates()
                    .as_ptr(),
                pointer
            );
            assert_eq!(
                plan.intent().int32().expect("Int32 intent").column(),
                case.column
            );
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16BeforeChangeInt32
            );
            assert_eq!(
                plan.intent().int32().expect("Int32 intent").updates().len(),
                case.updates.len()
            );
            for (actual, original) in plan
                .intent()
                .int32()
                .expect("Int32 intent")
                .updates()
                .iter()
                .zip(&case.updates)
            {
                assert_eq!(
                    (actual.row(), actual.expected(), actual.value()),
                    (original.ordinal, original.expected, original.replacement),
                );
            }
            let encoded = plan.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(&encoded[..8], b"TCNI0002");
            assert_eq!(&encoded[12..14], &2_u16.to_le_bytes());
            assert_eq!(
                encoded,
                intent(&case).encode(&mut budget(), &|| false).unwrap()
            );
            // RestoreOriginal retains the written proposal, not the restored result.
            if matches!(recipe, Recipe::FixedRestore | Recipe::CounterRestore) {
                assert_ne!(case.updates[0].expected, case.updates[0].replacement);
                assert_eq!(expected, &case.before);
                assert_eq!(plan.changed_rows(), 0);
            }
            if case.primary {
                assert_eq!(&plan.after_bytes()[9 * 4096..], &case.before[9 * 4096..]);
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
                &journals::journal(&case.before, expected, Marker::Committed),
            );
            images += 1;
        }
    }
    assert_eq!(images, 16);
}

#[test]
fn late_raise_and_generated_or_opaque_dormant_fields_refuse_without_a_plan() {
    let mut refusals = 0;
    for header in [5, 6] {
        for recipe in [
            Recipe::LateRaise,
            Recipe::DormantCounter,
            Recipe::UserCounter,
            Recipe::OpaqueOperand,
        ] {
            let case = model::source(header, recipe);
            assert!(case.expected.is_none());
            let error = record(&case, &mut budget(), &|| false).err().unwrap();
            match recipe {
                Recipe::LateRaise => {
                    let RecoverableEditError::Edit(EditError::BeforeChangeRejected(rejection)) =
                        error
                    else {
                        panic!("late RaiseError must retain its exact rejection");
                    };
                    assert_eq!(rejection.number(), 17);
                }
                Recipe::OpaqueOperand => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::ActiveContent)),
                )),
                _ => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column)),
                )),
            }
            // No journal/target resources are presented to the failed planner.
            assert_eq!(image(&case.before).as_bytes(), case.before);
            refusals += 1;
        }
    }
    assert_eq!(refusals, 8);
}

#[test]
fn empty_requests_still_compile_metadata_and_supplied_equal_or_stale_requests_refuse() {
    for header in [5, 6] {
        for (recipe, blocker) in [
            (Recipe::DormantCounter, WriteBlocker::Column),
            (Recipe::UserCounter, WriteBlocker::Column),
            (Recipe::OpaqueOperand, WriteBlocker::ActiveContent),
        ] {
            let case = model::source(header, recipe);
            assert!(matches!(
                image(&case.before).plan_recoverable_int32_updates(
                    "Items", case.column, Vec::new(), &mut budget(), &|| false,
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(actual))) if actual == blocker,
            ));
        }
        let case = model::source(header, Recipe::CounterPrimary);
        for updates in [
            vec![Int32Update::new(0, 1000, 1000)],
            vec![
                Int32Update::new(0, 1000, 17),
                Int32Update::new(1, 2000, 2000),
            ],
            vec![
                Int32Update::new(1, 2000, 2000),
                Int32Update::new(0, 1000, 17),
            ],
        ] {
            assert!(matches!(
                image(&case.before).plan_recoverable_int32_updates(
                    "Items",
                    "Score",
                    updates,
                    &mut budget(),
                    &|| false,
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::EventTiming
                ))),
            ));
        }
        assert!(matches!(
            image(&case.before).plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![Int32Update::new(0, 1001, 17)],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict)),
        ));
        let empty = model::source(header, Recipe::CounterEmpty);
        let plan = record(&empty, &mut budget(), &|| false).unwrap();
        assert!(
            plan.intent()
                .int32()
                .expect("Int32 intent")
                .updates()
                .is_empty()
        );
        assert_eq!(plan.after_bytes(), empty.before);
        assert_eq!(plan.changed_rows(), 0);
    }
}

#[test]
fn mixed_planning_pays_each_pool_and_final_cancel_before_publication() {
    for recipe in [Recipe::FixedSequential, Recipe::CounterPrimary] {
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
fn mixed_pending_committed_and_restarted_recovery_requalify_all_sixteen_images() {
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
                        intent(&case),
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
                // Confirmation uses the same original-only native qualifier.
                // Filesystem retired-namespace ownership is covered by Access/IO.
                target.fail_prefix = Some(0);
                journal.fail_prefix = Some(0);
                let writes = target.writes;
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent(&case),
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
    assert_eq!(images, 16);
}

#[test]
fn recovery_rejects_different_intent_program_or_index_bytes_and_stale_targets() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::CounterPrimary);
        let after = case.expected.as_ref().unwrap();
        for variant in 0..4 {
            let mut original = model::source(header, Recipe::CounterPrimary);
            let mut before = case.before.clone();
            let mut candidate = after.clone();
            match variant {
                0 => original.updates[0].replacement = 19,
                1 => before = model::source(header, Recipe::CounterRestore).before,
                2 => candidate[9 * 4096 + 700] ^= 1,
                _ => candidate[200] ^= 1,
            }
            let mut journal =
                Storage::new(journals::journal(&before, &candidate, Marker::Committed));
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let error = qualify_native_update_recovery(
                checked,
                intent(&original),
                scope(),
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
            if variant == 1 {
                assert!(matches!(error, NativeRecoveryQualificationError::Intent(_)));
            } else {
                assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::ImageMismatch
                ));
            }
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
fn mixed_recovery_pools_cancel_and_stale_complete_target_precede_writes() {
    let case = model::source(6, Recipe::CounterPrimary);
    let after = case.expected.as_ref().unwrap();
    let mut journal = Storage::new(journals::journal(&case.before, after, Marker::Pending));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let mut measured = budget();
    let initial = POOLS.map(|resource| measured.remaining(resource));
    let polls = Cell::new(0);
    drop(
        qualify_native_update_recovery(checked, intent(&case), scope(), &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
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
            intent(&case),
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
