//! Child of the existing qualified_recovery::api owner; storage and journal
//! framing remain owned by that already qualified test module.
use super::{Storage, budget, model};
use model::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, Int32Update, NativeImage, NativeInt32EditIntent,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, RecoverableNativeEditPlan, WriteBlocker, qualify_native_update_recovery,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access::canonical_before_change::{Case, RECIPES, Recipe, source};

const ADMITTED: [Recipe; 5] = [
    Recipe::Sequential,
    Recipe::MacroOnlyTarget,
    Recipe::RestoreOriginal,
    Recipe::Empty,
    Recipe::DormantDelete,
];
const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn scope() -> CatalogScope {
    CatalogScope::new(94)
}

fn requests(case: &Case) -> Vec<Int32Update> {
    case.updates
        .iter()
        .map(|update| Int32Update::new(update.ordinal, update.expected, update.replacement))
        .collect()
}

fn image(case: &Case) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(&case.before),
        scope(),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn plan(
    case: &Case,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RecoverableNativeEditPlan, RecoverableEditError> {
    image(case).plan_recoverable_int32_updates("Items", "Score", requests(case), work, cancelled)
}

// Construct request DATA without obtaining a plan. The closed decoder must
// still demand complete original-source qualification before recovery.
fn intent(before: &[u8], updates: Vec<Int32Update>) -> NativeUpdateIntent {
    let old =
        NativeInt32EditIntent::new("Items", "Score", updates, before, &mut budget(), &|| false)
            .unwrap();
    let mut wire = old.encode(&mut budget(), &|| false).unwrap();
    wire[..8].copy_from_slice(b"TCNI0002");
    wire[12..14].copy_from_slice(&2_u16.to_le_bytes());
    NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap()
}

// Independent derivative of the already qualified three-row fixed source.
// Only literal payload replacements are written; no native serializer, planner
// output, macro evaluator or candidate bytes participate in this expectation.
fn literal_image(case: &Case, changes: &[(usize, usize, i32, i32)]) -> Vec<u8> {
    let mut expected = case.before.clone();
    let page = &case.before[8 * 4096..9 * 4096];
    assert_eq!(&page[12..14], &3_u16.to_le_bytes());
    for &(row, column, before, after) in changes {
        assert!(row < 3 && column < 2);
        let word = 14 + 2 * row;
        let start = usize::from(u16::from_le_bytes(page[word..word + 2].try_into().unwrap()));
        assert_eq!(start & 0xe000, 0);
        let end = if row == 0 {
            4096
        } else {
            usize::from(u16::from_le_bytes(page[word - 2..word].try_into().unwrap()))
        };
        assert_eq!(end - start, 11);
        assert_eq!(&page[start..start + 2], &[2, 0]);
        assert_eq!(page[start + 10], 3);
        let payload = start + 2 + 4 * column;
        assert_eq!(&page[payload..payload + 4], &before.to_le_bytes());
        let absolute = 8 * 4096 + payload;
        expected[absolute..absolute + 4].copy_from_slice(&after.to_le_bytes());
    }
    expected
}

fn planning_inspection(error: RecoverableEditError) -> InspectError {
    match error {
        RecoverableEditError::Edit(EditError::Inspect(error))
        | RecoverableEditError::Intent(tablecraft_ace::NativeIntentError::Inspect(error))
        | RecoverableEditError::Intent(tablecraft_ace::NativeIntentError::Request(
            EditError::Inspect(error),
        )) => error,
        error => panic!("expected typed native inspection failure: {error:?}"),
    }
}

fn recovery_inspection(error: NativeRecoveryQualificationError) -> InspectError {
    match error {
        NativeRecoveryQualificationError::Inspect(error)
        | NativeRecoveryQualificationError::Edit(EditError::Inspect(error))
        | NativeRecoveryQualificationError::Intent(tablecraft_ace::NativeIntentError::Inspect(
            error,
        ))
        | NativeRecoveryQualificationError::Intent(tablecraft_ace::NativeIntentError::Request(
            EditError::Inspect(error),
        )) => error,
        error => panic!("expected typed recovery inspection failure: {error:?}"),
    }
}

#[test]
fn successful_multirow_requests_keep_original_order_and_count_distinct_roots() {
    for header in [5, 6] {
        let case = source(header, Recipe::Sequential);
        // Frozen XML: CurrentScore + OldScore, then CurrentScore + 1.
        // Row0: 17 + 10 + 1 = 28. Row2: 37 + 30 + 1 = 68.
        let expected = literal_image(&case, &[(0, 1, 10, 28), (2, 1, 30, 68)]);
        for reversed in [false, true] {
            let mut updates = vec![Int32Update::new(0, 10, 17), Int32Update::new(2, 30, 37)];
            if reversed {
                updates.reverse();
            }
            let recorded = image(&case)
                .plan_recoverable_int32_updates("Items", "Score", updates, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(recorded.before_bytes(), case.before);
            assert_eq!(recorded.after_bytes(), expected);
            assert_eq!(recorded.changed_rows(), 2);
            let ordinals = recorded
                .intent()
                .int32()
                .expect("Int32 intent")
                .updates()
                .iter()
                .map(Int32Update::row)
                .collect::<Vec<_>>();
            assert_eq!(ordinals, if reversed { [2, 0] } else { [0, 2] });
        }
    }
}

#[test]
fn alternate_user_id_target_and_macro_only_score_share_one_original_root() {
    for header in [5, 6] {
        let case = source(header, Recipe::Sequential);
        // The user seeds Id7. Score is not a user target: its Current10 plus
        // frozen Old10 then +1 yields21. Both cells belong to the same root.
        let expected = literal_image(&case, &[(0, 0, 1, 7), (0, 1, 10, 21)]);
        let recorded = image(&case)
            .plan_recoverable_int32_updates(
                "Items",
                "Id",
                vec![Int32Update::new(0, 1, 7)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(recorded.before_bytes(), case.before);
        assert_eq!(recorded.after_bytes(), expected);
        assert_eq!(recorded.changed_rows(), 1);
        assert_eq!(
            recorded.intent().int32().expect("Int32 intent").column(),
            "Id"
        );
        assert_eq!(
            recorded.intent().int32().expect("Int32 intent").updates()[0].expected(),
            1
        );
        assert_eq!(
            recorded.intent().int32().expect("Int32 intent").updates()[0].value(),
            7
        );
    }
}

#[test]
fn normal_planner_commits_ten_literal_images_and_retains_original_requests() {
    let mut images = 0;
    for header in [5, 6] {
        for recipe in ADMITTED {
            let case = source(header, recipe);
            let updates = requests(&case);
            let pointer = updates.as_ptr();
            let recorded = image(&case)
                .plan_recoverable_int32_updates("Items", "Score", updates, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(recorded.before_bytes(), case.before);
            assert_eq!(recorded.after_bytes(), case.expected.as_ref().unwrap());
            assert_eq!(recorded.changed_rows(), case.changed_rows);
            assert_eq!(
                recorded
                    .intent()
                    .int32()
                    .expect("Int32 intent")
                    .updates()
                    .as_ptr(),
                pointer
            );
            assert_eq!(
                recorded.intent().profile(),
                NativeUpdateProfile::Ace16BeforeChangeInt32
            );
            for (actual, original) in recorded
                .intent()
                .int32()
                .expect("Int32 intent")
                .updates()
                .iter()
                .zip(&case.updates)
            {
                assert_eq!(actual.row(), original.ordinal);
                assert_eq!(actual.expected(), original.expected);
                assert_eq!(actual.value(), original.replacement);
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            let prepared = recorded
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                prepared.commit(&mut budget(), &|| false).unwrap(),
                case.changed_rows
            );
            assert_eq!(target.bytes.get_ref(), case.expected.as_ref().unwrap());
            assert_eq!(
                journal.bytes.get_ref(),
                &model::journal(
                    &case.before,
                    case.expected.as_ref().unwrap(),
                    Marker::Committed
                ),
            );
            images += 1;
        }
    }
    assert_eq!(images, 10);
}

#[test]
fn unsupported_contexts_and_supplied_noops_never_return_a_normal_plan() {
    let mut refusals = 0;
    for header in [5, 6] {
        for recipe in RECIPES {
            if ADMITTED.contains(&recipe) {
                continue;
            }
            let case = source(header, recipe);
            let error = plan(&case, &mut budget(), &|| false)
                .err()
                .expect("unqualified source or original intent must refuse");
            if matches!(recipe, Recipe::UserNoopMacroChanges | Recipe::AllNoop) {
                assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::EventTiming)),
                ));
            } else if matches!(recipe, Recipe::Stale | Recipe::Duplicate) {
                assert!(matches!(
                    error,
                    RecoverableEditError::Edit(EditError::Conflict)
                ));
            } else if recipe == Recipe::LateRaise {
                let RecoverableEditError::Edit(EditError::BeforeChangeRejected(rejection)) = error
                else {
                    panic!("RaiseError must retain its typed native rejection");
                };
                assert_eq!(rejection.number(), 17);
            }
            refusals += 1;
        }
        // A journal-only positive route does not widen the old ordinary plan API.
        let case = source(header, Recipe::Sequential);
        assert!(matches!(
            image(&case).plan_int32_updates(
                "Items",
                "Score",
                &requests(&case),
                &mut budget(),
                &|| false,
            ),
            Err(EditError::Unsupported(WriteBlocker::Format)),
        ));
    }
    assert_eq!(refusals, 22);
    let case = source(6, Recipe::Sequential);
    for updates in [
        vec![Int32Update::new(0, 10, 17), Int32Update::new(1, 20, 20)],
        vec![Int32Update::new(1, 20, 20), Int32Update::new(0, 10, 17)],
    ] {
        assert!(matches!(
            image(&case).plan_recoverable_int32_updates(
                "Items",
                "Score",
                updates,
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::EventTiming,
            ))),
        ));
    }
}

#[test]
fn planning_exact_pools_and_final_cancellation_precede_plan_publication() {
    let case = source(6, Recipe::Sequential);
    let mut measured = budget();
    let initial = POOLS.map(|resource| measured.remaining(resource));
    let polls = Cell::new(0);
    drop(
        plan(&case, &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap(),
    );
    let used =
        std::array::from_fn::<_, 3, _>(|index| initial[index] - measured.remaining(POOLS[index]));
    assert!(used.iter().all(|value| *value > 0));
    plan(
        &case,
        &mut ResourceBudget::new(used[0], used[1], used[2]),
        &|| false,
    )
    .unwrap();
    for index in 0..3 {
        let mut below = used;
        below[index] -= 1;
        let error = plan(
            &case,
            &mut ResourceBudget::new(below[0], below[1], below[2]),
            &|| false,
        )
        .err()
        .expect("one-below native pool must refuse");
        assert!(matches!(
            planning_inspection(error),
            InspectError::Limit(error) if error.resource == POOLS[index],
        ));
    }
    let terminal = polls.get();
    polls.set(0);
    assert!(matches!(
        plan(&case, &mut budget(), &|| {
            polls.set(polls.get() + 1);
            polls.get() == terminal
        }),
        Err(RecoverableEditError::Edit(EditError::Inspect(
            InspectError::Cancelled
        ))) | Err(RecoverableEditError::Intent(
            tablecraft_ace::NativeIntentError::Inspect(InspectError::Cancelled,)
        )),
    ));
    assert_eq!(
        planning_inspection(plan(&case, &mut budget(), &|| true).err().unwrap()),
        InspectError::Cancelled,
    );
}

#[test]
fn stale_complete_target_refuses_before_target_or_journal_image_writes() {
    for header in [5, 6] {
        let case = source(header, Recipe::Sequential);
        let recorded = plan(&case, &mut budget(), &|| false).unwrap();
        let mut target = Storage::new(case.before.clone());
        // Unrelated header byte: whole BEFORE, not only the selected cell, binds prepare.
        target.bytes.get_mut()[200] ^= 1;
        let unchanged = target.bytes.get_ref().clone();
        let mut journal = Storage::new(Vec::new());
        assert!(matches!(
            recorded.prepare(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch),
        ));
        assert_eq!(target.bytes.get_ref(), &unchanged);
        assert_eq!(target.writes, 0);
        assert_eq!(journal.writes, 0);
        assert!(journal.bytes.get_ref().is_empty());
    }
}

#[test]
fn versioned_recovery_rebuilds_ten_images_for_pending_partial_and_committed_states() {
    for header in [5, 6] {
        for recipe in ADMITTED {
            let case = source(header, recipe);
            let after = case.expected.as_ref().unwrap();
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let mut journal = Storage::new(model::journal(&case.before, after, marker));
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
        }
    }
}

#[test]
fn interrupted_recovery_restarts_and_completed_confirmation_is_write_denied() {
    for header in [5, 6] {
        let case = source(header, Recipe::MacroOnlyTarget);
        let after = case.expected.as_ref().unwrap();
        let mut journal = Storage::new(model::journal(&case.before, after, Marker::Partial));
        let saved = journal.bytes.get_ref().clone();
        let mut target = Storage::new(after.clone());
        target.fail_prefix = Some(8 * 4096 + 4091);
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_update_recovery(
            checked,
            intent(&case.before, requests(&case)),
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
        assert!(target.writes > 0);
        // The first record's Id has rolled back but its Score is still17.
        assert_ne!(target.bytes.get_ref(), &case.before);
        assert_ne!(target.bytes.get_ref(), after);
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
            RecoveryOutcome::RolledBack,
        );
        assert_eq!(target.bytes.get_ref(), &case.before);
        assert_eq!(journal.bytes.get_ref(), &saved);
        assert_eq!(journal.writes, 0);

        // Retired namespace association belongs to Access/IO tests. Here the
        // same qualified complete journal performs write-free image confirmation.
        for marker in [Marker::Pending, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let mut journal = Storage::new(model::journal(&case.before, after, marker));
            let mut target = Storage::new(if committed {
                after.clone()
            } else {
                case.before.clone()
            });
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
                },
            );
            assert_eq!(target.writes, 0);
            assert_eq!(journal.writes, 0);
        }
    }
}

#[test]
fn recovery_rebinds_original_intent_program_bytes_and_entire_after_image() {
    for header in [5, 6] {
        let case = source(header, Recipe::Sequential);
        let after = case.expected.as_ref().unwrap();
        for variant in 0..6 {
            let mut before = case.before.clone();
            let mut candidate = after.clone();
            if variant == 3 {
                before = source(header, Recipe::MacroOnlyTarget).before;
            } else if variant == 4 {
                candidate[200] ^= 1;
            }
            let original = match variant {
                0 => intent(&case.before, vec![Int32Update::new(0, 11, 17)]),
                1 => intent(&case.before, vec![Int32Update::new(0, 10, 19)]),
                2 => {
                    let mut bound = case.before.clone();
                    bound[200] ^= 1;
                    intent(&bound, requests(&case))
                }
                5 => {
                    let old = NativeInt32EditIntent::new(
                        "Items",
                        "Score",
                        requests(&case),
                        &case.before,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    NativeUpdateIntent::decode(
                        &old.encode(&mut budget(), &|| false).unwrap(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                }
                _ => intent(&case.before, requests(&case)),
            };
            let mut journal = Storage::new(model::journal(&before, &candidate, Marker::Committed));
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let error =
                qualify_native_update_recovery(checked, original, scope(), &mut budget(), &|| {
                    false
                })
                .err()
                .expect("well-framed journal bytes are not native authority");
            match variant {
                0 => assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::Edit(EditError::Conflict)
                )),
                2 | 3 => assert!(matches!(error, NativeRecoveryQualificationError::Intent(_))),
                5 => assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::Edit(EditError::Unsupported(
                        WriteBlocker::Format,
                    )),
                )),
                _ => assert!(matches!(
                    error,
                    NativeRecoveryQualificationError::ImageMismatch
                )),
            }
            assert_eq!(journal.writes, 0);
        }
    }
}

#[test]
fn recovery_exact_pools_final_cancel_and_stale_target_never_write() {
    let case = source(6, Recipe::Sequential);
    let after = case.expected.as_ref().unwrap();
    let mut journal = Storage::new(model::journal(&case.before, after, Marker::Pending));
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let original = intent(&case.before, requests(&case));
    let mut measured = budget();
    let initial = POOLS.map(|resource| measured.remaining(resource));
    let polls = Cell::new(0);
    drop(
        qualify_native_update_recovery(checked, original, scope(), &mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap(),
    );
    let used =
        std::array::from_fn::<_, 3, _>(|index| initial[index] - measured.remaining(POOLS[index]));
    assert!(used.iter().all(|value| *value > 0));
    let terminal = polls.get();
    for limited in 0..5 {
        let mut limits = used;
        if limited < 3 {
            limits[limited] -= 1;
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        polls.set(0);
        let result = qualify_native_update_recovery(
            checked,
            intent(&case.before, requests(&case)),
            scope(),
            &mut ResourceBudget::new(limits[0], limits[1], limits[2]),
            &|| {
                polls.set(polls.get() + 1);
                limited == 4 && polls.get() == terminal
            },
        );
        if limited == 3 {
            let mut target = Storage::new(after.clone());
            target.bytes.get_mut()[200] ^= 1;
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
            let error = recovery_inspection(result.err().expect("limited recovery must refuse"));
            if limited < 3 {
                assert!(
                    matches!(error, InspectError::Limit(error) if error.resource == POOLS[limited])
                );
            } else {
                assert_eq!(error, InspectError::Cancelled);
            }
        }
        assert_eq!(journal.writes, 0);
    }
}

#[path = "normal/indexed.rs"]
mod indexed;

#[path = "normal/tree.rs"]
mod tree;

#[path = "normal/mixed.rs"]
mod mixed;

#[path = "normal/indexed_mixed.rs"]
mod indexed_mixed;

#[path = "normal/nullable_index.rs"]
mod nullable_index;

#[path = "normal/nullable_keys.rs"]
mod nullable_keys;

#[path = "normal/id_companion.rs"]
mod id_companion;

#[path = "normal/native_fixed.rs"]
mod native_fixed;

#[path = "normal/native_fixed_controls.rs"]
mod native_fixed_controls;

#[path = "normal/native_ordinary.rs"]
mod native_ordinary;

#[path = "normal/ordinary_inline.rs"]
mod ordinary_inline;

#[path = "normal/ordinary_counter_index.rs"]
mod ordinary_counter_index;

#[path = "normal/ordinary_indexed_targets.rs"]
mod ordinary_indexed_targets;

#[path = "normal/genuine_ordinary.rs"]
mod genuine_ordinary;

#[path = "normal/fitted.rs"]
mod fitted;

#[path = "normal/genuine_property_lval.rs"]
mod genuine_property_lval;

#[path = "normal/genuine_retained_numeric.rs"]
mod genuine_retained_numeric;

#[path = "normal/genuine_owned_growth.rs"]
mod genuine_owned_growth;

#[path = "normal/genuine_owned_growth_joint.rs"]
mod genuine_owned_growth_joint;

#[path = "normal/genuine_alternate_owned.rs"]
mod genuine_alternate_owned;

#[path = "normal/genuine_mixed_owned.rs"]
mod genuine_mixed_owned;

#[path = "normal/genuine_owned_growth_multirow.rs"]
mod genuine_owned_growth_multirow;

#[path = "normal/genuine_eof_allocation.rs"]
mod genuine_eof_allocation;

#[path = "normal/genuine_ordinary_delete.rs"]
mod genuine_ordinary_delete;

#[path = "normal/genuine_forwarded_delete.rs"]
mod genuine_forwarded_delete;

#[path = "normal/genuine_ordinary_insert.rs"]
mod genuine_ordinary_insert;

#[path = "normal/genuine_multirow_insert.rs"]
mod genuine_multirow_insert;

#[path = "normal/genuine_ordinary_insert_feff.rs"]
mod genuine_ordinary_insert_feff;

#[path = "normal/genuine_indexed_delete.rs"]
mod genuine_indexed_delete;

#[path = "normal/genuine_indexed_insert.rs"]
mod genuine_indexed_insert;

#[path = "normal/genuine_desc_nullable_delete.rs"]
mod genuine_desc_nullable_delete;

#[path = "normal/genuine_indexed_insert_association.rs"]
mod genuine_indexed_insert_association;

#[path = "normal/genuine_retained_node_delete.rs"]
mod genuine_retained_node_delete;

#[path = "normal/genuine_desc_nullable_insert.rs"]
mod genuine_desc_nullable_insert;

#[path = "normal/genuine_retained_node_insert.rs"]
mod genuine_retained_node_insert;

#[path = "normal/genuine_default_insert.rs"]
mod genuine_default_insert;

#[path = "normal/genuine_inline_lval_insert.rs"]
mod genuine_inline_lval_insert;

#[path = "normal/genuine_eof_insert.rs"]
mod genuine_eof_insert;

#[path = "normal/resizable_storage.rs"]
mod resizable_storage;

#[path = "normal/genuine_integral_indexed.rs"]
mod genuine_integral_indexed;

#[path = "normal/wide_complete_insert.rs"]
mod wide_complete_insert;

#[path = "normal/wide_primary_numeric.rs"]
mod wide_primary_numeric;

#[path = "normal/dense_numeric.rs"]
mod dense_numeric;

#[path = "normal/sparse_numeric.rs"]
mod sparse_numeric;

#[path = "normal/genuine_binary_indexed.rs"]
mod genuine_binary_indexed;

#[path = "normal/genuine_binary_delete.rs"]
mod genuine_binary_delete;

#[path = "normal/genuine_binary_update.rs"]
mod genuine_binary_update;
