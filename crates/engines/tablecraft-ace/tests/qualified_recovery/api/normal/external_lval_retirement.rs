//! Real TCNI5 commits and native recovery against independent external literals.
#[path = "external_lval_retirement/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, SlotScalarUpdate, WriteBlocker, qualify_native_update_recovery,
    read_table,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::{
    genuine_external_lval_continuation as fixtures,
    genuine_external_lval_update::{Case, Group, Update, binary},
};

fn cases() -> Vec<Case> {
    fixtures::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
    .into_iter()
    .filter(|case| {
        matches!(
            case.recipe,
            "clear-single-blob"
                | "clear-single-memo"
                | "clear-single-both"
                | "empty-single-blob"
                | "clear-memo-chain"
        )
    })
    .collect()
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFittedIntentGroup> {
    case.groups
        .iter()
        .map(|group| {
            let updates = group
                .updates
                .iter()
                .map(|cell| {
                    SlotScalarUpdate::new(cell.row, cell.expected.clone(), cell.replacement.clone())
                })
                .collect();
            NativeFittedIntentGroup::new(
                group.column,
                NativeFittedUpdates::Variable(updates),
                &mut budget(),
                &|| false,
            )
            .unwrap()
        })
        .collect()
}

fn before_digest(before: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    hash.finalize().into()
}

#[test]
#[ignore = "requires external continuation36 source admission before matching producers"]
fn external_retirement_updates_commit_exact_literals_with_original_profile_five_groups() {
    let cases = cases();
    let wires = cases
        .iter()
        .map(|case| fixtures::intent_wire(case, before_digest(&case.before)))
        .collect::<Vec<_>>();
    let mut commits = 0;
    for (case, original_wire) in cases.iter().zip(wires) {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fitted_updates("Items", groups(case), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), case.changed);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryFitted
        );
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            original_wire
        );
        let decoded = NativeUpdateIntent::decode(&original_wire, &mut budget(), &|| false).unwrap();
        assert_eq!(
            decoded.encode(&mut budget(), &|| false).unwrap(),
            original_wire
        );
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            case.changed
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed)
        );
        assert_eq!(
            read_table(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            case.expected.as_slice()
        );
        commits += 1;
    }
    assert_eq!(commits, 20);
}

#[test]
#[ignore = "requires external continuation36 source admission before matching producers"]
fn external_retirement_original_intent_recovery_preserves_partial_and_committed_laws() {
    let mut contexts = 0;
    for case in cases().into_iter().filter(|case| case.changed != 0) {
        let differences = case
            .before
            .iter()
            .zip(&case.after)
            .enumerate()
            .filter_map(|(at, (old, new))| (old != new).then_some(at))
            .collect::<Vec<_>>();
        assert!(differences.len() > 1);
        let mut mixed = case.before.clone();
        mixed[differences[0]] = case.after[differences[0]];
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for initial in [&case.after, &mixed] {
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mismatch = committed && initial != &case.after;
                let expected = if mismatch {
                    initial
                } else if committed {
                    &case.after
                } else {
                    &case.before
                };
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let mut target = Storage::new(initial.clone());
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.after, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let intent = NativeUpdateIntent::decode(
                        &fixtures::intent_wire(&case, before_digest(&case.before)),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let recovered = qualified.recover(&mut target, &mut budget(), &|| false);
                    if mismatch {
                        assert!(matches!(recovered, Err(JournalError::TargetMismatch)));
                    } else {
                        assert_eq!(recovered.unwrap(), outcome);
                    }
                    assert_eq!(target.bytes.get_ref(), expected);
                }
                if committed {
                    assert_eq!(target.writes, 0);
                }
                let writes = target.writes;
                target.fail_prefix = Some(0);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent = NativeUpdateIntent::decode(
                    &fixtures::intent_wire(&case, before_digest(&case.before)),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let confirmed = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if mismatch {
                    assert!(matches!(confirmed, Err(JournalError::TargetMismatch)));
                } else {
                    assert_eq!(confirmed.unwrap(), outcome);
                }
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 120);
}

#[test]
#[ignore = "requires external continuation36 source admission before matching producers"]
fn external_retirement_residuals_orphans_and_map_masks_are_not_recovery_authority() {
    let mut refused = 0;
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.recipe, "clear-single-both" | "clear-memo-chain"))
    {
        let delta = usize::from(case.header == 6) * 17;
        let retired = fixtures::retired_pages(&case);
        let map_change = case
            .before
            .iter()
            .zip(&case.after)
            .enumerate()
            .find_map(|(at, (old, new))| {
                let page = at / 4096;
                (old != new && page != case.data as usize && !retired.contains(&(page as u32)))
                    .then_some(at)
            })
            .unwrap();
        for at in [
            retired[0] as usize * 4096,
            retired[0] as usize * 4096 + 4095,
            (103 + delta) * 4096 + 4095,
            case.definition as usize * 4096 + 20,
            map_change,
        ] {
            let mut forged = case.after.clone();
            forged[at] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent = NativeUpdateIntent::decode(
                &fixtures::intent_wire(&case, before_digest(&case.before)),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            refused += 1;
        }
    }
    assert_eq!(refused, 40);
}

#[test]
#[ignore = "requires continuation36 Source and matching whole-page retirement"]
fn external_retirement_rejects_unselected_shared_rows_and_stale_original_expectations() {
    let mut shared = 0;
    let mut stale = 0;
    for mut case in cases()
        .into_iter()
        .filter(|case| case.recipe == "clear-single-both")
    {
        for (row, expected) in [(4, binary(4077, 5)), (5, binary(65, 6))] {
            let group = NativeFittedIntentGroup::new(
                "Blob",
                NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                    row,
                    expected,
                    Value::Null,
                )]),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let error = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    vec![group],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::RowLayout))
            ));
            shared += 1;
        }
        case.groups[0].updates[0].expected = Value::Null;
        let expected_wire = fixtures::intent_wire(&case, before_digest(&case.before));
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_fitted_updates(
                "Items",
                groups(&case),
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let saved = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = NativeUpdateIntent::decode(&expected_wire, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
            Err(NativeRecoveryQualificationError::Edit(EditError::Conflict))
        ));
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);
        stale += 1;
    }
    assert_eq!((shared, stale), (8, 4));
}

#[test]
#[ignore = "requires continuation36 Source and matching whole-page retirement"]
fn external_retirement_result_noops_and_empty_groups_preserve_exact_source_relative_images() {
    let mut commits = 0;
    for mut case in cases()
        .into_iter()
        .filter(|case| case.recipe == "clear-single-both")
    {
        case.before = case.after.clone();
        for empty in [false, true] {
            case.groups = vec![Group {
                column: "Memo",
                updates: if empty {
                    Vec::new()
                } else {
                    vec![Update {
                        row: 2,
                        expected: Value::Null,
                        replacement: Value::Null,
                    }]
                },
            }];
            let expected_wire = fixtures::intent_wire(&case, before_digest(&case.before));
            let plan = image(&case.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.changed_rows(), 0);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                expected_wire
            );
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                0
            );
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert!(target.writes > 0);
            commits += 1;
        }
    }
    assert_eq!(commits, 8);
}
