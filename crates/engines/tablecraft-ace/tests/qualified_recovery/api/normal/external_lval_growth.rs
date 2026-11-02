//! Real TCNI5 commits and native recovery against independent external literals.
#[path = "external_lval_growth/resources.rs"]
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
    genuine_external_lval_update::{Case, Group, Update, binary, original_rows},
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
            "grow-shared-last-blob" | "grow-chain-last-memo" | "joint-terminal-growth"
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
fn external_terminal_growth_updates_commit_exact_literals_with_original_profile_five_groups() {
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
    assert_eq!(commits, 12);
}

#[test]
#[ignore = "requires external continuation36 source admission before matching producers"]
fn external_terminal_growth_original_intent_recovery_preserves_partial_and_committed_laws() {
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
    assert_eq!(contexts, 72);
}

#[test]
#[ignore = "requires external continuation36 source admission before matching producers"]
fn external_growth_chunks_slack_and_preserved_lval_declarations_are_not_recovery_authority() {
    let mut refused = 0;
    for case in cases() {
        let delta = usize::from(case.header == 6) * 17;
        for at in [
            (95 + delta) * 4096 + 4031,
            (100 + delta) * 4096 + 4087,
            (103 + delta) * 4096 + 4095,
            case.definition as usize * 4096 + 20,
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
    assert_eq!(refused, 48);
}

#[test]
#[ignore = "requires external continuation36 native/SDK qualification before matching"]
fn external_growth_rejects_native_body_cap_and_actual_shared_page_capacity() {
    let mut contexts = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "grow-shared-last-blob")
    {
        for (row, expected, replacement) in [
            (3, binary(4076, 4), binary(4077, 24)),
            (5, binary(65, 6), binary(4070, 26)),
        ] {
            let group = NativeFittedIntentGroup::new(
                "Blob",
                NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                    row,
                    expected,
                    replacement,
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
            contexts += 1;
        }
    }
    assert_eq!(contexts, 8);
}

#[test]
#[ignore = "requires continuation36 qualification before matching terminal growth"]
fn external_growth_keeps_original_noops_empty_groups_and_stale_intents_closed() {
    let mut commits = 0;
    let mut refused = 0;
    for mut case in cases()
        .into_iter()
        .filter(|case| case.recipe == "grow-shared-last-blob")
    {
        for empty in [false, true] {
            let old_value = binary(65, 6);
            let original = Case {
                profile: case.profile,
                header: case.header,
                recipe: case.recipe,
                definition: case.definition,
                data: case.data,
                before: case.before.clone(),
                after: case.before.clone(),
                expected: original_rows(),
                groups: vec![Group {
                    column: "Blob",
                    updates: if empty {
                        Vec::new()
                    } else {
                        vec![Update {
                            row: 5,
                            expected: old_value.clone(),
                            replacement: old_value,
                        }]
                    },
                }],
                changed: 0,
            };
            let expected_wire = fixtures::intent_wire(&original, before_digest(&original.before));
            let plan = image(&original.before)
                .plan_recoverable_ordinary_fitted_updates(
                    "Items",
                    groups(&original),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.changed_rows(), 0);
            assert_eq!(plan.after_bytes(), original.before);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                expected_wire
            );
            let mut target = Storage::new(original.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                0
            );
            // Zero logical changes still follow the durable image commit.
            // Its synchronized target write must preserve the complete bytes.
            assert_eq!(target.bytes.get_ref(), &original.before);
            assert!(target.writes > 0);
            commits += 1;
        }
        case.groups[0].updates[0].expected = Value::Binary(vec![0; 65]);
        let original_wire = fixtures::intent_wire(&case, before_digest(&case.before));
        let error = image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "Items",
                groups(&case),
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RecoverableEditError::Edit(EditError::Conflict)
        ));
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let saved = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = NativeUpdateIntent::decode(&original_wire, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
            Err(NativeRecoveryQualificationError::Edit(EditError::Conflict))
        ));
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);
        refused += 1;
    }
    assert_eq!(commits, 8);
    assert_eq!(refused, 4);
}
