//! Child of qualified_recovery::api::normal, reusing its actual storage,
//! framing, closed intent and typed error helpers without copying any model.
use super::{
    Storage, budget, intent, model as journal_model, planning_inspection, recovery_inspection,
    scope,
};
use journal_model::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    BeforeEventIntents, EditError, IndexCheckStatus, IndexConstraintViolation, InspectError,
    Int32Update, NativeImage, NativeRecoveryQualificationError, RecoverableNativeEditPlan,
    WriteBlocker, check_table, qualify_native_update_recovery, read_table, read_table_index,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::canonical_indexed_before_change::{self as corpus, Case, Recipe};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn updates(case: &Case) -> Vec<Int32Update> {
    case.updates
        .iter()
        .map(|value| Int32Update::new(value.ordinal, value.expected, value.replacement))
        .collect()
}

fn record(
    case: &Case,
    work: &mut ResourceBudget,
    cancel: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, tablecraft_ace::RecoverableEditError> {
    image(&case.before).plan_recoverable_int32_updates(
        "Items",
        "Score",
        updates(case),
        work,
        cancel,
    )
}

#[test]
fn indexed_normal_plans_commit_all_sixteen_literal_images_and_reverse_the_unique_swap() {
    let mut positives = 0;
    for header in [5, 6] {
        for recipe in corpus::RECIPES {
            let case = corpus::source(header, recipe);
            let Some(expected) = &case.expected else {
                continue;
            };
            let mut orders = vec![false];
            if recipe == Recipe::UniqueSwap {
                orders.push(true);
            }
            for reversed in orders {
                let mut original = updates(&case);
                if reversed {
                    original.reverse();
                }
                let recorded = image(&case.before)
                    .plan_recoverable_int32_updates(
                        "Items",
                        "Score",
                        original,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(recorded.before_bytes(), case.before);
                assert_eq!(recorded.after_bytes(), expected);
                assert_eq!(recorded.changed_rows(), case.changed_rows);
                let actual = recorded
                    .intent()
                    .int32()
                    .expect("Int32 intent")
                    .updates()
                    .iter()
                    .map(Int32Update::row)
                    .collect::<Vec<_>>();
                let mut order = case
                    .updates
                    .iter()
                    .map(|value| value.ordinal)
                    .collect::<Vec<_>>();
                if reversed {
                    order.reverse();
                }
                assert_eq!(actual, order);
                let mut target = Storage::new(case.before.clone());
                let mut journal = Storage::new(Vec::new());
                let prepared = recorded
                    .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    prepared.commit(&mut budget(), &|| false).unwrap(),
                    case.changed_rows
                );
                assert_eq!(target.bytes.get_ref(), expected);
                assert_eq!(
                    journal.bytes.get_ref(),
                    &journal_model::journal(&case.before, expected, Marker::Committed)
                );
            }
            positives += 1;
        }
    }
    assert_eq!(positives, 16);
}

// Derivative expectations are encoded entirely before invoking any planner.
// Frozen CurrentScore9 + Old10 +1 gives20, tying row1's original Score20.
fn tie_expectation(case: &Case, composite: bool) -> Vec<u8> {
    let mut expected = case.before.clone();
    let word = 8 * 4096 + 14;
    let start = 8 * 4096
        + usize::from(u16::from_le_bytes(
            case.before[word..word + 2].try_into().unwrap(),
        ));
    assert_eq!(&case.before[start..start + 2], &[2, 0]);
    assert_eq!(&case.before[start + 6..start + 10], &10_i32.to_le_bytes());
    expected[start + 6..start + 10].copy_from_slice(&20_i32.to_le_bytes());
    // ASC Score ties retain original ROW0 before ROW1. Composite Id DESC puts
    // Id2/ROW1 before Id1/ROW0, proving the second component changes ordering.
    let ordered = if composite {
        [(2_i32, 20_i32, 1_u32), (1, 20, 0), (3, 30, 2)]
    } else {
        [(1_i32, 20_i32, 0_u32), (2, 20, 1), (3, 30, 2)]
    };
    let mut wire = Vec::new();
    for (id, score, slot) in ordered {
        wire.push(0x7f);
        wire.extend_from_slice(&((score as u32) ^ 0x8000_0000).to_be_bytes());
        if composite {
            wire.push(0x80);
            wire.extend_from_slice(&((id as u32) ^ 0x8000_0000).to_be_bytes().map(|byte| !byte));
        }
        wire.extend_from_slice(&((8_u32 << 8) | slot).to_be_bytes());
    }
    let length = if composite { 42 } else { 27 };
    assert_eq!(wire.len(), length);
    expected[9 * 4096 + 480..9 * 4096 + 480 + length].copy_from_slice(&wire);
    assert_eq!(&expected[..8 * 4096], &case.before[..8 * 4096]);
    assert_eq!(
        &expected[9 * 4096..9 * 4096 + 480],
        &case.before[9 * 4096..9 * 4096 + 480]
    );
    assert_eq!(
        &expected[9 * 4096 + 480 + length..],
        &case.before[9 * 4096 + 480 + length..]
    );
    expected
}

#[test]
fn nonunique_ties_and_composite_descending_tie_order_match_prequalified_literal_images() {
    for header in [5, 6] {
        for (recipe, composite) in [(Recipe::Ascending, false), (Recipe::Composite, true)] {
            let case = corpus::source(header, recipe);
            let expected = tie_expectation(&case, composite);
            // Strong readonly qualification precedes the producer call below.
            let qualified = image(&expected);
            let checked = check_table(
                &mut qualified.reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            let rows = read_table(
                &mut qualified.reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                rows.rows(),
                &[
                    vec![Value::Int32(1), Value::Int32(20)],
                    vec![Value::Int32(2), Value::Int32(20)],
                    vec![Value::Int32(3), Value::Int32(30)],
                ]
            );
            let ordered = read_table_index(
                &mut qualified.reader(),
                scope(),
                "Items",
                "EventKey",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let ids = ordered
                .rows()
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>();
            assert_eq!(
                ids,
                if composite { [2, 1, 3] } else { [1, 2, 3] }.map(Value::Int32)
            );
            let bound = qualified
                .prepare_native_before_change_fields(
                    scope().object_id(7),
                    BeforeEventIntents::FixedUpdate {
                        column: "Score",
                        updates: &[],
                    },
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert!(bound.final_rows().unwrap().is_empty());
            assert_eq!(qualified.as_bytes(), expected);
            let recorded = image(&case.before)
                .plan_recoverable_int32_updates(
                    "Items",
                    "Score",
                    vec![Int32Update::new(0, 10, 9)],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(recorded.after_bytes(), expected);
            assert_eq!(recorded.changed_rows(), 1);
        }
    }
}

#[test]
fn unique_conflicts_and_supplied_equal_assignments_keep_typed_refusals() {
    for header in [5, 6] {
        let case = corpus::source(header, Recipe::UniqueConflict);
        assert!(case.expected.is_none());
        assert!(matches!(
            record(&case, &mut budget(), &|| false),
            Err(tablecraft_ace::RecoverableEditError::Edit(
                EditError::Inspect(InspectError::IndexConstraint(
                    IndexConstraintViolation::DuplicateKey
                ),)
            )),
        ));
        let case = corpus::source(header, Recipe::Ascending);
        assert!(matches!(
            image(&case.before).plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![Int32Update::new(0, 10, 10)],
                &mut budget(),
                &|| false
            ),
            Err(tablecraft_ace::RecoverableEditError::Edit(
                EditError::Unsupported(WriteBlocker::EventTiming)
            )),
        ));
    }
}

#[test]
fn indexed_recovery_qualifies_all_original_intents_for_pending_and_committed_journals() {
    let mut positives = 0;
    for header in [5, 6] {
        for recipe in corpus::RECIPES {
            let case = corpus::source(header, recipe);
            let Some(after) = &case.expected else {
                continue;
            };
            for marker in [Marker::Pending, Marker::Committed] {
                let mut journal = Storage::new(journal_model::journal(&case.before, after, marker));
                let saved = journal.bytes.get_ref().clone();
                let mut target = Storage::new(after.clone());
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent(&case.before, updates(&case)),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let outcome = qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap();
                    if matches!(marker, Marker::Committed) {
                        assert_eq!(outcome, RecoveryOutcome::Committed);
                        assert_eq!(target.bytes.get_ref(), after);
                        assert_eq!(target.writes, 0);
                    } else {
                        assert_eq!(outcome, RecoveryOutcome::RolledBack);
                        assert_eq!(target.bytes.get_ref(), &case.before);
                    }
                }
                assert_eq!(journal.bytes.get_ref(), &saved);
                assert_eq!(journal.writes, 0);
            }
            positives += 1;
        }
    }
    assert_eq!(positives, 16);
}

#[test]
fn indexed_rollback_restarts_and_index_only_forgery_or_stale_target_never_gains_authority() {
    let case = corpus::source(6, Recipe::Ascending);
    let after = case.expected.as_ref().unwrap();
    let mut journal = Storage::new(journal_model::journal(&case.before, after, Marker::Partial));
    let saved = journal.bytes.get_ref().clone();
    let mut target = Storage::new(after.clone());
    target.fail_prefix = Some(9 * 4096 + 485);
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_native_update_recovery(
        checked,
        intent(&case.before, updates(&case)),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .is_err()
    );
    assert_ne!(target.bytes.get_ref(), &case.before);
    assert_ne!(target.bytes.get_ref(), after);
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_native_update_recovery(
        checked,
        intent(&case.before, updates(&case)),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::RolledBack
    );
    assert_eq!(target.bytes.get_ref(), &case.before);
    assert_eq!(journal.bytes.get_ref(), &saved);
    assert_eq!(journal.writes, 0);
    for offset in [9 * 4096 + 484, 9 * 4096 + 4095] {
        let mut forged = after.clone();
        forged[offset] ^= 1;
        let mut journal = Storage::new(journal_model::journal(
            &case.before,
            &forged,
            Marker::Committed,
        ));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_update_recovery(
                checked,
                intent(&case.before, updates(&case)),
                scope(),
                &mut budget(),
                &|| false
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(journal.writes, 0);
    }
    let recorded = record(&case, &mut budget(), &|| false).unwrap();
    let mut target = Storage::new(case.before.clone());
    target.bytes.get_mut()[9 * 4096 + 4095] ^= 1;
    let stale = target.bytes.get_ref().clone();
    let mut journal = Storage::new(Vec::new());
    assert!(matches!(
        recorded.prepare(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::TargetMismatch)
    ));
    assert_eq!(target.bytes.get_ref(), &stale);
    assert_eq!((target.writes, journal.writes), (0, 0));
    assert!(journal.bytes.get_ref().is_empty());
}

#[test]
fn indexed_planning_and_recovery_keep_exact_pools_and_final_cancellation() {
    let case = corpus::source(6, Recipe::Composite);
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
    let used =
        std::array::from_fn::<_, 3, _>(|index| initial[index] - measured.remaining(POOLS[index]));
    assert!(used.iter().all(|count| *count > 0));
    record(
        &case,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    for index in 0..3 {
        let mut below = used;
        below[index] -= 1;
        let error = planning_inspection(
            record(
                &case,
                &mut ResourceBudget::new(below[0], below[1], below[2]),
                &|| false,
            )
            .err()
            .unwrap(),
        );
        assert!(matches!(error, InspectError::Limit(error) if error.resource == POOLS[index]));
    }
    let terminal = polls.get();
    polls.set(0);
    let error = planning_inspection(
        record(&case, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        })
        .err()
        .unwrap(),
    );
    assert_eq!(error, InspectError::Cancelled);
    let mut journal = Storage::new(journal_model::journal(
        &case.before,
        case.expected.as_ref().unwrap(),
        Marker::Pending,
    ));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let original = intent(&case.before, updates(&case));
    let mut measured = budget();
    let initial = POOLS.map(|resource| measured.remaining(resource));
    polls.set(0);
    drop(
        qualify_native_update_recovery(checked, original, scope(), &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap(),
    );
    let used =
        std::array::from_fn::<_, 3, _>(|index| initial[index] - measured.remaining(POOLS[index]));
    let terminal = polls.get();
    for limit in 0..5 {
        let mut below = used;
        if limit < 3 {
            below[limit] -= 1;
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        polls.set(0);
        let result = qualify_native_update_recovery(
            checked,
            intent(&case.before, updates(&case)),
            scope(),
            &mut ResourceBudget::new(below[0], below[1], below[2]),
            &|| {
                polls.set(polls.get() + 1);
                limit == 4 && polls.get() == terminal
            },
        );
        if limit == 3 {
            drop(result.unwrap());
        } else {
            let error = recovery_inspection(result.err().unwrap());
            if limit < 3 {
                assert!(
                    matches!(error, InspectError::Limit(error) if error.resource == POOLS[limit])
                );
            } else {
                assert_eq!(error, InspectError::Cancelled);
            }
        }
        assert_eq!(journal.writes, 0);
    }
}
