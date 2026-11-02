//! Child of qualified_recovery::api::normal; reuse its storage and framing.
use super::{
    Storage, budget, intent, model as journals, planning_inspection, recovery_inspection, scope,
};
use journals::Marker;
use std::{cell::Cell, io::Cursor};
use tablecraft_ace::{
    EditError, InspectError, Int32Update, NativeImage, NativeRecoveryQualificationError,
    NativeUpdateProfile, RecoverableEditError, RecoverableNativeEditPlan, WriteBlocker,
    check_table, qualify_native_update_recovery,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::canonical_tree_before_change::{
    self as model, Case, Recipe,
};

const POOLS: [Resource; 3] = [Resource::Bytes, Resource::Rows, Resource::Steps];

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn updates(case: &Case) -> Vec<Int32Update> {
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
        updates(case),
        work,
        cancel,
    )
}

#[test]
fn normal_tree_plans_commit_eighteen_literal_images_without_page_or_map_allocation() {
    let mut count = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            let plan = record(&case, &mut budget(), &|| false).unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.expected);
            assert_eq!(plan.changed_rows(), case.changed_rows);
            assert_eq!(plan.after_bytes().len(), case.before.len());
            assert_eq!(&plan.after_bytes()[..8 * 4096], &case.before[..8 * 4096]);
            assert_eq!(
                plan.intent().profile(),
                NativeUpdateProfile::Ace16BeforeChangeInt32
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
                    (original.ordinal, original.expected, original.replacement)
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
            assert_eq!(target.bytes.get_ref(), &case.expected);
            assert_eq!(target.bytes.get_ref().len(), case.before.len());
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.expected, Marker::Committed)
            );
            count += 1;
        }
    }
    assert_eq!(count, 18);
}

#[test]
fn normal_prefix_loss_fits_while_existing_jet4_keeps_its_fixed_encoding_contract() {
    for header in [5, 6] {
        for (recipe, page) in [(Recipe::LeafPrefixLoss, 9), (Recipe::NodePrefixLoss, 10)] {
            let case = model::source(header, recipe);
            assert_eq!(
                &case.before[page * 4096 + 24..page * 4096 + 26],
                &4_u16.to_le_bytes()
            );
            assert_eq!(
                &case.expected[page * 4096 + 24..page * 4096 + 26],
                &1_u16.to_le_bytes()
            );
            let plan = record(&case, &mut budget(), &|| false).unwrap();
            assert_eq!(plan.after_bytes(), case.expected);
            assert_eq!(plan.after_bytes().len(), case.before.len());
        }
    }
    let jet = journals::case(journals::Shape::Ascending);
    let plan = image(&jet.before)
        .plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, 12)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(plan.after_bytes(), jet.after);
    assert_eq!(plan.intent().profile(), NativeUpdateProfile::Jet4Int32);
    assert_eq!(
        &plan.intent().encode(&mut budget(), &|| false).unwrap()[..8],
        b"TCNI0001"
    );
    assert!(matches!(
        image(&jet.before).plan_recoverable_int32_updates(
            "Items",
            "Score",
            vec![Int32Update::new(0, 10, i32::MAX)],
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::IndexedColumn
        ))),
    ));
}

#[test]
fn retained_upper_bounds_real_restoration_empty_and_supplied_equal_intent_are_distinct() {
    for header in [5, 6] {
        let case = model::source(header, Recipe::NodeRetainedUpper);
        assert_eq!(
            &case.before[9 * 4096 + 480..9 * 4096 + 493],
            &[0x7f, 0x80, 0, 0, 25, 0, 0, 8, 1, 0, 0, 0, 10]
        );
        let restore = record(&case, &mut budget(), &|| false).unwrap();
        assert_eq!(restore.after_bytes(), case.before);
        assert_eq!(restore.changed_rows(), 0);
        assert_eq!(
            restore
                .intent()
                .int32()
                .expect("Int32 intent")
                .updates()
                .len(),
            1
        );
        assert_eq!(
            restore.intent().int32().expect("Int32 intent").updates()[0].value(),
            17
        );
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

        // A changed non-key field must also retain this index's loose bound.
        // Construct and read-check the entire expected image before planning.
        let mut expected = case.before.clone();
        let word = 8 * 4096 + 14;
        let start = 8 * 4096
            + usize::from(u16::from_le_bytes(
                case.before[word..word + 2].try_into().unwrap(),
            ));
        assert_eq!(&expected[start + 2..start + 6], &1_i32.to_le_bytes());
        expected[start + 2..start + 6].copy_from_slice(&17_i32.to_le_bytes());
        check_table(
            &mut image(&expected).reader(),
            scope(),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let changed = image(&case.before)
            .plan_recoverable_int32_updates(
                "Items",
                "Id",
                vec![Int32Update::new(0, 1, 17)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(changed.after_bytes(), expected);
        assert_eq!(changed.changed_rows(), 1);

        assert!(matches!(
            image(&case.before).plan_recoverable_int32_updates(
                "Items",
                "Score",
                vec![Int32Update::new(0, 10, 10)],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::EventTiming
            )))
        ));
        let mut unqualified = case.before.clone();
        unqualified[9 * 4096 + 8] = 1;
        assert!(matches!(
            image(&unqualified).plan_recoverable_int32_updates(
                "Items",
                "Score",
                Vec::new(),
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::IndexedColumn
            )))
        ));
    }
}

#[test]
fn original_intents_requalify_all_eighteen_tree_pairs_for_pending_and_committed_recovery() {
    let mut count = 0;
    for header in [5, 6] {
        for recipe in model::RECIPES {
            let case = model::source(header, recipe);
            for marker in [Marker::Pending, Marker::Committed] {
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.expected, marker));
                let saved = journal.bytes.get_ref().clone();
                let mut target = Storage::new(case.expected.clone());
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
                        assert_eq!(target.bytes.get_ref(), &case.expected);
                        assert_eq!(target.writes, 0);
                    } else {
                        assert_eq!(outcome, RecoveryOutcome::RolledBack);
                        assert_eq!(target.bytes.get_ref(), &case.before);
                    }
                }
                assert_eq!(journal.bytes.get_ref(), &saved);
                assert_eq!(journal.writes, 0);
            }
            count += 1;
        }
    }
    assert_eq!(count, 18);
}

#[test]
fn recovery_restarts_from_mixed_prefix_pages_and_rejects_node_prefix_or_tail_tampering() {
    let case = model::source(6, Recipe::NodePrefixLoss);
    let mut journal = Storage::new(journals::journal(
        &case.before,
        &case.expected,
        Marker::Partial,
    ));
    let saved = journal.bytes.get_ref().clone();
    let mut target = Storage::new(case.expected.clone());
    target.fail_prefix = Some(10 * 4096 + 24);
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
    assert_ne!(target.bytes.get_ref(), &case.expected);
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
    for offset in [9 * 4096 + 20, 10 * 4096 + 24, 11 * 4096 + 4095] {
        let mut forged = case.expected.clone();
        forged[offset] ^= 1;
        let mut journal = Storage::new(journals::journal(&case.before, &forged, Marker::Committed));
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
    let plan = record(&case, &mut budget(), &|| false).unwrap();
    let mut target = Storage::new(case.before.clone());
    target.bytes.get_mut()[11 * 4096 + 4095] ^= 1;
    let unchanged = target.bytes.get_ref().clone();
    let mut journal = Storage::new(Vec::new());
    assert!(matches!(
        plan.prepare(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::TargetMismatch)
    ));
    assert_eq!(target.bytes.get_ref(), &unchanged);
    assert_eq!((target.writes, journal.writes), (0, 0));
    assert!(journal.bytes.get_ref().is_empty());
}

#[test]
fn tree_fitting_planning_and_recovery_preserve_exact_pools_and_final_cancellation() {
    let case = model::source(6, Recipe::NodePrefixLoss);
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
    assert!(used.iter().all(|value| *value > 0));
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
    assert_eq!(
        planning_inspection(
            record(&case, &mut budget(), &|| {
                polls.set(polls.get() + 1);
                polls.get() == terminal
            })
            .err()
            .unwrap()
        ),
        InspectError::Cancelled
    );
    let mut journal = Storage::new(journals::journal(
        &case.before,
        &case.expected,
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
