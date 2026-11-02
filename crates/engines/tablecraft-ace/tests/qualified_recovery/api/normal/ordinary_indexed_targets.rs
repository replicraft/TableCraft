//! Intended child of qualified_recovery::api::normal. Private, unexecuted draft.
use super::{Storage, budget, model as journals, planning_inspection, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, IndexConstraintViolation, InspectError, NativeFixedIntentGroup,
    NativeImage, NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent,
    NativeUpdateProfile, RecoverableEditError, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::canonical_ordinary_indexed_targets::{
    self as sources, Case, Recipe, Refusal,
};

fn value(value: Option<i32>) -> Value {
    value.map_or(Value::Null, Value::Int32)
}

fn groups(case: &Case) -> Vec<NativeFixedIntentGroup> {
    vec![
        NativeFixedIntentGroup::new(
            "Id",
            case.updates
                .iter()
                .map(|request| {
                    FixedScalarUpdate::new(
                        request.row,
                        value(request.expected),
                        value(request.replacement),
                    )
                })
                .collect(),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn carrier(bytes: &mut Vec<u8>, value: Option<i32>) {
    bytes.push(if value.is_some() { 2 } else { 0 });
    bytes.extend_from_slice(
        &value
            .map_or(0, |value| u64::from(value as u32))
            .to_le_bytes(),
    );
}

fn wire(case: &Case) -> Vec<u8> {
    let mut wire = Vec::from(*b"TCNI0004");
    let total = 91 + 26 * case.updates.len();
    wire.extend_from_slice(&(total as u32).to_le_bytes());
    wire.extend_from_slice(&[4, 0, 0, 0]);
    wire.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((case.before.len() as u64).to_le_bytes());
    hash.update(&case.before);
    wire.extend_from_slice(&hash.finalize());
    for number in [5_u32, 1, case.updates.len() as u32, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(b"Items");
    for number in [2_u32, case.updates.len() as u32, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(b"Id");
    for request in &case.updates {
        wire.extend_from_slice(&(request.row as u64).to_le_bytes());
        carrier(&mut wire, request.expected);
        carrier(&mut wire, request.replacement);
    }
    assert_eq!(wire.len(), total);
    wire
}

fn intent(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

#[test]
fn twenty_four_literal_indexed_outcomes_commit_with_exact_original_v4_requests() {
    let mut committed = 0;
    for header in [5, 6] {
        for recipe in sources::RECIPES {
            let case = sources::source(header, recipe);
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
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.changed_rows(), case.changed_rows);
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16OrdinaryFixed
            );
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case)
            );
            let original = plan.intent().fixed().unwrap();
            assert_eq!(original.groups().len(), 1);
            assert_eq!(original.groups()[0].column(), "Id");
            for (actual, request) in original.groups()[0].updates().iter().zip(&case.updates) {
                assert_eq!(actual.row(), request.row);
                assert_eq!(actual.expected(), &value(request.expected));
                assert_eq!(actual.value(), &value(request.replacement));
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
            committed += 1;
        }
    }
    assert_eq!(committed, 24);
}

#[test]
fn indexed_null_tree_and_alias_recovery_rebind_original_requests_in_every_state() {
    for header in [5, 6] {
        for recipe in [
            Recipe::PromoteNull,
            Recipe::ClearValue,
            Recipe::UniqueSwap,
            Recipe::ManualTree,
            Recipe::Aliases,
            Recipe::NullNoop,
        ] {
            let case = sources::source(header, recipe);
            let after = case.expected.as_ref().unwrap();
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let mut target = Storage::new(after.clone());
                let mut journal = Storage::new(journals::journal(&case.before, after, marker));
                let saved = journal.bytes.get_ref().clone();
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
                        outcome
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
                    outcome
                );
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
            }
        }
    }
}

#[test]
fn indexed_requests_keep_exact_unique_generated_stale_primary_and_duplicate_refusals() {
    for header in [5, 6] {
        for recipe in [
            Recipe::UniqueConflict,
            Recipe::StaleNullExpected,
            Recipe::GeneratedTarget,
            Recipe::PrimaryNull,
        ] {
            let case = sources::source(header, recipe);
            let error = image(&case.before)
                .plan_recoverable_ordinary_fixed_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .expect("the frozen refusal request cannot return a plan");
            match case.refusal.unwrap() {
                Refusal::DuplicateKey => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::DuplicateKey
                    )))
                )),
                Refusal::Conflict => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Conflict)
                )),
                Refusal::GeneratedColumn => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column))
                )),
                Refusal::RequiredIndexKey => assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(
                        IndexConstraintViolation::NullKey
                    )))
                )),
            }
        }
        assert!(matches!(
            NativeFixedIntentGroup::new(
                "Id",
                vec![
                    FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(7)),
                    FixedScalarUpdate::new(0, Value::Int32(1), Value::Int32(8)),
                ],
                &mut budget(),
                &|| false
            ),
            Err(NativeIntentError::Request(EditError::Conflict))
        ));
        let case = sources::source(header, Recipe::ManualSeven);
        let error = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates("Items", groups(&case), &mut budget(), &|| {
                true
            })
            .err()
            .expect("immediate cancellation cannot return a plan");
        assert!(matches!(
            planning_inspection(error),
            InspectError::Cancelled
        ));
    }
}

#[test]
fn recovery_rejects_forged_index_body_and_count_without_touching_resources() {
    for header in [5, 6] {
        let case = sources::source(header, Recipe::ManualSeven);
        for variant in 0..3 {
            let mut after = case.expected.as_ref().unwrap().clone();
            let original = if variant == 2 {
                let mut altered = sources::source(header, Recipe::ManualSeven);
                altered.updates[0].replacement = Some(8);
                intent(&altered)
            } else {
                if variant == 0 {
                    after[9 * 4096 + 480] ^= 1;
                }
                if variant == 1 {
                    after[8 * 4096 + 14] ^= 1;
                }
                intent(&case)
            };
            let mut journal =
                Storage::new(journals::journal(&case.before, &after, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, original, scope(), &mut budget(), &|| {
                    false
                }),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
        let mut count = wire(&case);
        count[64..68].copy_from_slice(&2_u32.to_le_bytes());
        assert!(matches!(
            NativeUpdateIntent::decode(&count, &mut budget(), &|| false),
            Err(NativeIntentError::InvalidRecord)
        ));
        let mut reversed = sources::source(header, Recipe::UniqueSwap);
        reversed.updates.reverse();
        let plan = image(&reversed.before)
            .plan_recoverable_ordinary_fixed_updates(
                "Items",
                groups(&reversed),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), reversed.expected.as_ref().unwrap());
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&reversed)
        );
    }
}
