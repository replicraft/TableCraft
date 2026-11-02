//! Original TCNI5 requests and independent full-image Text source outcomes.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile, SlotScalarUpdate,
    qualify_native_update_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_text_storage_carriers::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(case: &Case) -> Vec<NativeFittedIntentGroup> {
    let updates = case
        .updates()
        .into_iter()
        .map(|(row, before, after)| SlotScalarUpdate::new(row, before, after))
        .collect();
    vec![
        NativeFittedIntentGroup::new(
            "KeyValue",
            NativeFittedUpdates::Variable(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn carrier(output: &mut Vec<u8>, value: &Value) {
    let (tag, bytes) = match value {
        Value::Null => (0, &[][..]),
        Value::Text(text) => (6, text.as_bytes()),
        _ => panic!("literal Text/NULL source only"),
    };
    output.extend_from_slice(&[tag, 0, 0, 0]);
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn wire(case: &Case) -> Vec<u8> {
    let updates = case.updates();
    let mut cells = Vec::new();
    for (row, expected, value) in &updates {
        let mut cell = (*row as u64).to_le_bytes().to_vec();
        carrier(&mut cell, expected);
        carrier(&mut cell, value);
        cells.extend_from_slice(&((cell.len() + 4) as u32).to_le_bytes());
        cells.extend_from_slice(&cell);
    }
    let mut body = case.table.as_bytes().to_vec();
    body.extend_from_slice(&((28 + cells.len()) as u32).to_le_bytes());
    body.extend_from_slice(&8_u32.to_le_bytes());
    body.extend_from_slice(&(updates.len() as u32).to_le_bytes());
    body.extend_from_slice(&[1, 0, 0, 0]);
    body.extend_from_slice(&0_u32.to_le_bytes());
    body.extend_from_slice(b"KeyValue");
    body.extend_from_slice(&cells);
    let mut bytes = b"TCNI0005".to_vec();
    bytes.extend_from_slice(&((72 + body.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(&[5, 0, 0, 0]);
    bytes.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    bytes.extend_from_slice(&digest.finalize());
    for count in [case.table.len() as u32, 1, updates.len() as u32, 0] {
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    bytes.extend_from_slice(&body);
    bytes
}

fn intent(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires independently qualified Text sources and actual readonly Source admission"]
fn fitting_text_updates_commit_sixteen_carrier_contexts_with_exact_original_tcni5() {
    let cases = cases();
    let wires = cases.iter().map(wire).collect::<Vec<_>>();
    let mut committed = 0;
    for (case, expected_wire) in cases.iter().zip(wires) {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                case.table,
                groups(case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), case.changed_rows());
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryFitted
        );
        assert!(plan.intent().int32().is_none() && plan.intent().fixed().is_none());
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            expected_wire
        );
        let body = plan.intent().fitted().unwrap();
        assert_eq!(body.table(), case.table);
        let [group] = body.groups() else {
            panic!("one original Text group");
        };
        assert_eq!(group.column(), "KeyValue");
        let NativeFittedUpdates::Variable(updates) = group.updates() else {
            panic!("variable original family");
        };
        let expected = case.updates();
        assert_eq!(updates.len(), expected.len());
        for (actual, (row, before, after)) in updates.iter().zip(expected) {
            assert_eq!(
                (actual.row(), actual.expected(), actual.value()),
                (row, &before, &after)
            );
        }
        let mut target = Storage::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            case.changed_rows()
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        for (alias, name) in ["Primary", "Asc", "Twin", "Desc"].into_iter().enumerate() {
            let rows = tablecraft_ace::read_table_index(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), case.ordered_rows(true, alias));
            let physical = [0, 1, 1, 2][alias];
            let history = tablecraft_ace::inspect_table_index_counter(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(history.value(), case.histories[physical]);
        }
        let writes = target.writes;
        target.fail_prefix = Some(0);
        let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_update_recovery(checked, intent(case), scope(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!(target.writes, writes);
        committed += 1;
    }
    assert_eq!(committed, 16);
}

#[test]
#[ignore = "requires source-backed Text body/index mixtures and original TCNI5 replay"]
fn text_native_recovery_replays_both_general_families_and_denies_forged_after_before_effects() {
    let mut recovered = 0;
    for case in cases() {
        let mut mixed = case.before.clone();
        let partial = case.offset + 6;
        mixed[partial..partial + 2].copy_from_slice(&case.after[partial..partial + 2]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for (marker, initial) in [
            (Marker::Pending, case.before.clone()),
            (Marker::Partial, mixed),
            (Marker::Committed, case.after.clone()),
        ] {
            let committed = matches!(marker, Marker::Committed);
            let mut target = Storage::new(initial);
            let mut log = Storage::new(journals::journal(&case.before, &case.after, marker));
            let expected = if committed { &case.after } else { &case.before };
            for _ in 0..2 {
                let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
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
                assert_eq!(target.bytes.get_ref(), expected);
            }
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_update_recovery(
                checked,
                intent(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
            recovered += 1;
        }
        // A committed marker cannot authorize a target that still mixes
        // original and final page origins. Both recovery and confirmation
        // must validate the complete target before any durable effect.
        let mut committed_mixed = case.before.clone();
        committed_mixed[partial..partial + 2].copy_from_slice(&case.after[partial..partial + 2]);
        let mut target = Storage::new(committed_mixed.clone());
        let mut log = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Committed,
        ));
        let saved = log.bytes.get_ref().clone();
        let effects = (target.writes, log.writes);
        target.fail_prefix = Some(0);
        log.fail_prefix = Some(0);
        let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_update_recovery(checked, intent(&case), scope(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert!(matches!(
            qualified.recover(&mut target, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        ));
        let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_update_recovery(checked, intent(&case), scope(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert!(matches!(
            qualified.confirm_completed(&mut target, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        ));
        assert_eq!((target.writes, log.writes), effects);
        assert_eq!(target.bytes.get_ref(), &committed_mixed);
        assert_eq!(log.bytes.get_ref(), &saved);

        for at in [
            0,
            case.definition as usize * 4096 + 79,
            case.roots[1] as usize * 4096 + 4095,
            case.data as usize * 4096 + 4095,
        ] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let mut log = Storage::new(journals::journal(&case.before, &after, Marker::Pending));
            let saved = log.bytes.get_ref().clone();
            let target = Storage::new(case.before.clone());
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(log.bytes.get_ref(), &saved);
            assert_eq!((target.bytes.get_ref(), target.writes), (&case.before, 0));
        }
        let updates = vec![SlotScalarUpdate::new(
            4,
            Value::Text("A".into()),
            Value::Text("a".into()),
        )];
        let group = NativeFittedIntentGroup::new(
            "KeyValue",
            NativeFittedUpdates::Variable(updates),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_fitted_updates(
                case.table,
                vec![group],
                &mut budget(),
                &|| false
            ),
            Err(tablecraft_ace::RecoverableEditError::Edit(
                EditError::Conflict
            ))
        ));
    }
    assert_eq!(recovered, 48);
}

#[path = "text_storage_carriers/resources.rs"]
mod resources;
