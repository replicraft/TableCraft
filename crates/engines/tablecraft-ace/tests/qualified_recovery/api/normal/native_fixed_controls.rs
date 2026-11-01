//! Separate child of qualified_recovery::api::normal; original positive tests stay frozen.
use super::{Storage, budget, model as journals, planning_inspection, recovery_inspection, scope};
use journals::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, FixedScalarUpdate, InspectError, NativeFixedEditIntent, NativeFixedIntentGroup,
    NativeImage, NativeRecoveryQualificationError, NativeUpdateIntent, RecoverableEditError,
    RecoverableNativeEditPlan, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::{
    canonical_fixed_before_change::{self as model, Case, Recipe},
    canonical_nullable_fixed_int32_before_change as nullable,
};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn groups(case: &Case) -> Vec<NativeFixedIntentGroup> {
    case.groups
        .iter()
        .map(|group| {
            let name = model::FIELDS
                .iter()
                .find(|field| field.0 == group.column)
                .unwrap()
                .1;
            let requests = group
                .updates
                .iter()
                .map(|update| {
                    FixedScalarUpdate::new(
                        update.ordinal,
                        update.expected.value(),
                        update.replacement.value(),
                    )
                })
                .collect();
            NativeFixedIntentGroup::new(name, requests, &mut budget(), &|| false).unwrap()
        })
        .collect()
}

fn record(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
    // Capture and caller-owned request setup intentionally use separate pools;
    // measured work below is exactly the public recoverable planning operation.
    let image = NativeImage::capture(
        &mut Cursor::new(&case.before),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    image.plan_recoverable_fixed_updates("Items", groups(case), work, cancelled)
}

fn original(case: &Case) -> NativeUpdateIntent {
    // Data-only intent construction from SOURCE, without a planner or candidate.
    let original =
        NativeFixedEditIntent::new("Items", groups(case), &case.before, &mut budget(), &|| {
            false
        })
        .unwrap();
    let wire = original.encode(&mut budget(), &|| false).unwrap();
    NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap()
}

fn used(initial: [u64; 3], work: &ResourceBudget) -> [u64; 3] {
    std::array::from_fn(|index| initial[index] - work.remaining(POOLS[index]))
}

#[test]
fn normal_fixed_all_five_refusal_contexts_keep_their_typed_errors() {
    let mut refusals = 0;
    for header in [5, 6] {
        for recipe in [
            Recipe::EqualDate,
            Recipe::EqualInactiveDate,
            Recipe::WrongCarrier,
            Recipe::StaleLate,
            Recipe::DuplicateColumn,
        ] {
            let case = model::source(header, recipe);
            assert!(case.expected.is_none());
            let error = record(&case, &mut budget(), &|| false)
                .err()
                .expect("source refusal");
            match recipe {
                Recipe::EqualDate | Recipe::EqualInactiveDate => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::EventTiming))
                )),
                Recipe::WrongCarrier | Recipe::DuplicateColumn => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column))
                )),
                Recipe::StaleLate => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Conflict)
                )),
                _ => unreachable!(),
            }
            refusals += 1;
        }
    }
    assert_eq!(refusals, 10);
}

#[test]
fn normal_fixed_planning_exact_pools_and_terminal_cancellation_are_enforced() {
    let case = model::source(6, Recipe::JointSharedMask);
    let mut measured = budget();
    let initial = POOLS.map(|resource| measured.remaining(resource));
    let polls = Cell::new(0);
    let plan = record(&case, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    assert_eq!(plan.after_bytes(), case.expected.as_ref().unwrap());
    let costs = used(initial, &measured);
    assert!(costs.iter().all(|cost| *cost > 0));
    let terminal = polls.get();
    assert!(terminal > 1);
    for variant in 0..6 {
        let mut limits = costs;
        if variant < 3 {
            limits[variant] -= 1;
        }
        polls.set(0);
        let result = record(
            &case,
            &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
            &|| {
                polls.set(polls.get() + 1);
                variant == 4 || (variant == 5 && polls.get() == terminal)
            },
        );
        if variant == 3 {
            assert_eq!(
                result.unwrap().after_bytes(),
                case.expected.as_ref().unwrap()
            );
        } else {
            let error = planning_inspection(result.err().unwrap());
            if variant < 3 {
                assert!(
                    matches!(error, InspectError::Limit(error) if error.resource == POOLS[variant])
                );
            } else {
                assert_eq!(error, InspectError::Cancelled);
            }
        }
    }
}

#[test]
fn normal_fixed_recovery_exact_pools_and_terminal_cancellation_precede_writes() {
    let case = model::source(6, Recipe::JointSharedMask);
    let expected = case.expected.as_ref().unwrap();
    let mut journal = Storage::new(journals::journal(&case.before, expected, Marker::Pending));
    let saved = journal.bytes.get_ref().clone();
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let mut measured = budget();
    let initial = POOLS.map(|resource| measured.remaining(resource));
    let polls = Cell::new(0);
    drop(
        qualify_native_update_recovery(checked, original(&case), scope(), &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap(),
    );
    let costs = used(initial, &measured);
    assert!(costs.iter().all(|cost| *cost > 0));
    let terminal = polls.get();
    assert!(terminal > 1);
    for variant in 0..6 {
        let mut limits = costs;
        if variant < 3 {
            limits[variant] -= 1;
        }
        polls.set(0);
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let result = qualify_native_update_recovery(
            checked,
            original(&case),
            scope(),
            &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
            &|| {
                polls.set(polls.get() + 1);
                variant == 4 || (variant == 5 && polls.get() == terminal)
            },
        );
        if variant == 3 {
            // Qualification at all three exact limits succeeds. Actual recovery
            // still checks the complete live target, including unrelated bytes.
            let mut target = Storage::new(expected.clone());
            target.bytes.get_mut()[200] ^= 1;
            let retained = target.bytes.get_ref().clone();
            assert_eq!(
                result
                    .unwrap()
                    .recover(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            );
            assert_eq!(target.writes, 0);
            assert_eq!(target.bytes.get_ref(), &retained);
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
        assert_eq!(journal.bytes.get_ref(), &saved);
    }
}

#[test]
fn normal_fixed_stale_complete_target_refuses_prepare_without_any_writes() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::InactiveDateSibling);
        let (inactive, _) = model::record(&case.before, 1);
        for at in [200, inactive + 37] {
            let plan = record(&case, &mut budget(), &|| false).unwrap();
            let mut target = Storage::new(case.before.clone());
            target.bytes.get_mut()[at] ^= 1;
            let saved = target.bytes.get_ref().clone();
            let mut journal = Storage::new(Vec::new());
            assert!(matches!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            ));
            assert_eq!(target.writes, 0);
            assert_eq!(target.bytes.get_ref(), &saved);
            assert_eq!(journal.writes, 0);
            assert!(journal.bytes.get_ref().is_empty());
        }
    }
}

#[test]
fn normal_fixed_journal_after_must_equal_the_complete_original_request_replay() {
    let mut refusals = 0;
    for header in [5, 6] {
        let case = model::source(header, Recipe::InactiveDateSibling);
        let expected = case.expected.as_ref().unwrap();
        let (first, end) = model::record(&case.before, 0);
        let (inactive, _) = model::record(&case.before, 1);
        assert_eq!(&case.before[inactive + 37..inactive + 45], &[0xff; 8]);
        for marker in [Marker::Pending, Marker::Committed] {
            for variant in 0..6 {
                let mut candidate = expected.clone();
                match variant {
                    0 => candidate.copy_from_slice(&case.before),
                    1 => candidate[200] ^= 1,
                    2 => candidate[end - 1] ^= 2, // Unrelated GUID presence.
                    3 => candidate[inactive + 37] ^= 1,
                    4 => candidate[first + 17] ^= 1, // Exact scaled Currency.
                    _ => candidate[end - 2] ^= 1,    // Native Boolean value bit.
                }
                assert_ne!(candidate, *expected);
                let mut journal = Storage::new(journals::journal(&case.before, &candidate, marker));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let error = qualify_native_update_recovery(
                    checked,
                    original(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("causal complete-image refusal");
                assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::ImageMismatch
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                refusals += 1;
            }
        }
    }
    assert_eq!(refusals, 24);
}

#[test]
fn nullable_int32_fixed_requests_preserve_inactive_cells_in_both_directions() {
    let mut committed_images = 0;
    for header in [5, 6] {
        for recipe in [nullable::Recipe::Activate, nullable::Recipe::Clear] {
            let case = nullable::source(header, recipe);
            let make_groups = || {
                vec![
                    NativeFixedIntentGroup::new(
                        "Id",
                        vec![FixedScalarUpdate::new(
                            0,
                            case.expected_id.map_or(Value::Null, Value::Int32),
                            case.replacement_id.map_or(Value::Null, Value::Int32),
                        )],
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap(),
                ]
            };
            let image = NativeImage::capture(
                &mut Cursor::new(&case.before),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let plan = image
                .plan_recoverable_fixed_updates("Items", make_groups(), &mut budget(), &|| false)
                .unwrap();
            assert_eq!(plan.after_bytes(), case.expected);
            assert_eq!(plan.changed_rows(), 1);
            assert_eq!(
                plan.intent().fixed().unwrap().groups()[0].updates()[0].expected(),
                &case.expected_id.map_or(Value::Null, Value::Int32)
            );
            assert_eq!(
                plan.intent().fixed().unwrap().groups()[0].updates()[0].value(),
                &case.replacement_id.map_or(Value::Null, Value::Int32)
            );
            let original_wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = plan
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
            assert_eq!(target.bytes.get_ref(), &case.expected);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.expected, Marker::Committed)
            );
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.expected, marker));
                let mut target = Storage::new(case.expected.clone());
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let intent =
                        NativeUpdateIntent::decode(&original_wire, &mut budget(), &|| false)
                            .unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let outcome = if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    };
                    assert_eq!(
                        qualified
                            .recover(&mut target, &mut budget(), &|| false)
                            .unwrap(),
                        outcome
                    );
                    let expected = if committed {
                        &case.expected
                    } else {
                        &case.before
                    };
                    assert_eq!(target.bytes.get_ref(), expected);
                }
            }
            committed_images += 1;
            let (start, end) = nullable::record(&case.before);
            assert_eq!(
                &case.before[start + 2..start + 10],
                &[1, 0, 0, 0, 0xe8, 3, 0, 0]
            );
            if matches!(recipe, nullable::Recipe::Clear) {
                assert_eq!(
                    &case.expected[start + 2..start + 10],
                    &case.before[start + 2..start + 10]
                );
                assert_eq!(case.expected[end - 1] & 0x21, 0);
            }
            // Altering an inactive Score byte is still a whole-image mismatch;
            // logical NULL never grants permission to rewrite physical padding.
            let mut after = case.expected.clone();
            after[start + 6] ^= 1;
            let data = NativeFixedEditIntent::new(
                "Items",
                make_groups(),
                &case.before,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let wire = data.encode(&mut budget(), &|| false).unwrap();
            let intent = NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            let mut journal =
                Storage::new(journals::journal(&case.before, &after, Marker::Committed));
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
        }
    }
    assert_eq!(committed_images, 4);
}
