//! Real TCNI5 commits and native recovery against independent external literals.
#[path = "external_lval_update/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeRecoveryQualificationError,
    NativeUpdateIntent, NativeUpdateProfile, SlotScalarUpdate, qualify_native_update_recovery,
    read_table,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_external_lval_update::{self as fixtures, Case};

fn cases() -> Vec<Case> {
    fixtures::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
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

fn carrier(output: &mut Vec<u8>, value: &Value) {
    let (tag, payload) = match value {
        Value::Null => (0, Vec::new()),
        Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
        Value::Text(value) => (6, value.as_bytes().to_vec()),
        Value::Binary(value) => (7, value.clone()),
        _ => panic!("only original literal source carriers enter this wire"),
    };
    output.extend_from_slice(&[tag, 0, 0, 0]);
    output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    output.extend_from_slice(&payload);
}

fn wire(case: &Case) -> Vec<u8> {
    let mut body = Vec::from(*b"Items");
    let mut assignments = 0_u32;
    for group in &case.groups {
        let mut cells = Vec::new();
        for update in &group.updates {
            let mut cell = (update.row as u64).to_le_bytes().to_vec();
            carrier(&mut cell, &update.expected);
            carrier(&mut cell, &update.replacement);
            cells.extend_from_slice(&((cell.len() + 4) as u32).to_le_bytes());
            cells.extend_from_slice(&cell);
            assignments += 1;
        }
        body.extend_from_slice(&((20 + group.column.len() + cells.len()) as u32).to_le_bytes());
        body.extend_from_slice(&(group.column.len() as u32).to_le_bytes());
        body.extend_from_slice(&(group.updates.len() as u32).to_le_bytes());
        body.extend_from_slice(&[1, 0, 0, 0]);
        body.extend_from_slice(&0_u32.to_le_bytes());
        body.extend_from_slice(group.column.as_bytes());
        body.extend_from_slice(&cells);
    }
    let mut raw = Vec::from(*b"TCNI0005");
    raw.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    raw.extend_from_slice(&[5, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((case.before.len() as u64).to_le_bytes());
    hash.update(&case.before);
    raw.extend_from_slice(&hash.finalize());
    for number in [5, case.groups.len() as u32, assignments, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(&body);
    raw
}

#[test]
#[ignore = "requires external UPDATE32 source admission before matching producers"]
fn external_long_updates_commit_exact_literals_with_original_profile_five_groups() {
    let cases = cases();
    let wires = cases.iter().map(wire).collect::<Vec<_>>();
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
    assert_eq!(commits, 28);
}

#[test]
#[ignore = "requires external UPDATE32 source admission before matching producers"]
fn external_long_original_intent_recovery_preserves_partial_and_committed_laws() {
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
                    let intent =
                        NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
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
                let intent =
                    NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
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
#[ignore = "requires external UPDATE32 source admission before matching producers"]
fn external_chunks_slack_and_preserved_lval_declarations_are_not_recovery_authority() {
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
            let intent =
                NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            refused += 1;
        }
    }
    assert_eq!(refused, 112);
}

#[test]
#[ignore = "requires external UPDATE32 source admission before matching producers"]
fn external_topology_changes_and_stale_expected_values_refuse_before_journals() {
    let mut refused = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "empty-group")
    {
        for (column, row, expected, replacement) in [
            (
                "Blob",
                4,
                fixtures::binary(4077, 5),
                fixtures::binary(4075, 15),
            ),
            (
                "Memo",
                4,
                Value::Text("Ā".repeat(2039)),
                Value::Text("β".repeat(2036)),
            ),
            (
                "Blob",
                3,
                fixtures::binary(4076, 4),
                fixtures::binary(4077, 14),
            ),
            ("Blob", 4, fixtures::binary(4077, 5), Value::Null),
        ] {
            let group = NativeFittedIntentGroup::new(
                column,
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
                tablecraft_ace::RecoverableEditError::Edit(tablecraft_ace::EditError::Unsupported(
                    tablecraft_ace::WriteBlocker::RowLayout
                ))
            ));
            refused += 1;
        }
        let group = NativeFittedIntentGroup::new(
            "Memo",
            NativeFittedUpdates::Variable(vec![SlotScalarUpdate::new(
                4,
                Value::Text("stale".into()),
                Value::Text("β".repeat(2039)),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let error = image(&case.before)
            .plan_recoverable_ordinary_fitted_updates("Items", vec![group], &mut budget(), &|| {
                false
            })
            .err()
            .unwrap();
        assert!(matches!(
            error,
            tablecraft_ace::RecoverableEditError::Edit(tablecraft_ace::EditError::Conflict)
        ));
        refused += 1;
    }
    assert_eq!(refused, 20);
}
