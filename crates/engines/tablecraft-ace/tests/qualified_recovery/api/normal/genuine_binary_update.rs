//! Public original TCNI5 Binary updates, actual V1 commits and same-token recovery.
use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeFittedIntentGroup, NativeFittedUpdates, NativeImage, NativeRecoveryQualificationError,
    NativeUpdateIntent, SlotScalarUpdate, qualify_native_update_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1, prepare_expected};
use tablecraft_testkit::access::genuine_binary_update::{self as source, Case};

fn cases() -> Vec<Case> {
    source::load_all(|raw, hash| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            hash
        )
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(case: &Case) -> Vec<NativeFittedIntentGroup> {
    let updates = source::updates(case.recipe)
        .into_iter()
        .map(|(row, old, new)| SlotScalarUpdate::new(row, old, new))
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

fn carrier(value: &Value) -> Vec<u8> {
    let (tag, bytes) = match value {
        Value::Null => (0, &[][..]),
        Value::Binary(bytes) => (7, bytes.as_slice()),
        _ => panic!("outside frozen original Binary request family"),
    };
    let mut wire = vec![tag, 0, 0, 0];
    wire.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    wire.extend_from_slice(bytes);
    wire
}

fn wire(case: &Case) -> Vec<u8> {
    let updates = source::updates(case.recipe);
    let mut body = Vec::new();
    for (ordinal, old, new) in &updates {
        let before = carrier(old);
        let after = carrier(new);
        body.extend_from_slice(&((12 + before.len() + after.len()) as u32).to_le_bytes());
        body.extend_from_slice(&(*ordinal as u64).to_le_bytes());
        body.extend_from_slice(&before);
        body.extend_from_slice(&after);
    }
    let group_length = 20 + 8 + body.len();
    let total = 72 + 10 + group_length;
    let mut result = Vec::from(*b"TCNI0005");
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&[5, 0, 0, 0]);
    result.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    result.extend_from_slice(&digest.finalize());
    for value in [10_u32, 1, updates.len() as u32, 0] {
        result.extend_from_slice(&value.to_le_bytes());
    }
    result.extend_from_slice(b"BinaryKeys");
    for value in [group_length as u32, 8, updates.len() as u32] {
        result.extend_from_slice(&value.to_le_bytes());
    }
    result.extend_from_slice(&[1, 0, 0, 0]);
    result.extend_from_slice(&0_u32.to_le_bytes());
    result.extend_from_slice(b"KeyValue");
    result.extend_from_slice(&body);
    assert_eq!(result.len(), total);
    result
}

fn intent(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

fn journal(case: &Case, after: &[u8], marker: usize) -> Storage {
    let mut target = Storage::new(case.before.clone());
    let mut log = Storage::new(Vec::new());
    let prepared = prepare_expected(
        &mut target,
        &mut log,
        &case.before,
        after,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    if marker == 2 {
        prepared.commit(&mut budget(), &|| false).unwrap();
    } else {
        drop(prepared);
    }
    if marker == 1 {
        log.bytes.get_mut().extend_from_slice(b"COM");
    }
    log
}

#[test]
#[ignore = "requires 16 qualified fitting Binary UPDATE views"]
fn binary_fitting_update_commits_use_original_independent_tcni5_wire() {
    for case in cases() {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fitted_updates(
                "BinaryKeys",
                requests(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
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
        assert_eq!(&log.bytes.get_ref()[..8], b"TCIMG001");
    }
}

#[test]
#[ignore = "requires 16 qualified fitting Binary UPDATE views"]
fn binary_update_recovery_qualifies_same_v1_token_and_rejects_protected_after_forgery() {
    let mut contexts = 0;
    let mut forgeries = 0;
    for case in cases() {
        let data = case.data as usize * 4096;
        let mut mixed = case.before.clone();
        mixed[data..data + 4096].copy_from_slice(&case.after[data..data + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for marker in [0, 1, 2] {
            for initial in [case.after.clone(), mixed.clone()] {
                let refuse = marker == 2 && initial == mixed;
                let mut target = Storage::new(initial.clone());
                let mut log = journal(&case, &case.after, marker);
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
                    let writes = target.writes;
                    let outcome = qualified.recover(&mut target, &mut budget(), &|| false);
                    if refuse {
                        assert!(matches!(outcome, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.bytes.get_ref(), &initial);
                        assert_eq!(target.writes, writes);
                    } else {
                        assert_eq!(
                            outcome.unwrap(),
                            if marker == 2 {
                                RecoveryOutcome::Committed
                            } else {
                                RecoveryOutcome::RolledBack
                            }
                        );
                        assert_eq!(
                            target.bytes.get_ref(),
                            if marker == 2 {
                                &case.after
                            } else {
                                &case.before
                            }
                        );
                    }
                }
                contexts += 1;
            }
        }
        for at in [
            data + 4095,
            data + 4088,
            case.definition as usize * 4096 + 79,
            case.roots[1] as usize * 4096 + 484,
            case.roots[2] as usize * 4096 + 4000,
            4096 + 100,
        ] {
            let mut forged = case.after.clone();
            forged[at] ^= 1;
            let mut log = journal(&case, &forged, 0);
            let saved = log.bytes.get_ref().clone();
            let writes = log.writes;
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
            assert_eq!(log.writes, writes);
            forgeries += 1;
        }
    }
    assert_eq!((contexts, forgeries), (84, 84));
}
