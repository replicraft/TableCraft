//! Frozen original/model root-split images, actual TCNA4 and V2 lifecycle.
use super::{Storage, budget, resizable_storage::Target, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeCompleteInsertIntent, NativeImage, NativeRecoveryQualificationError, RowInsert,
    qualify_native_complete_insert_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{
    JournalError, RecoveryOutcome, inspect_image, prepare_resizable_expected,
};
use tablecraft_testkit::fixtures::access::genuine_root_split::{self as sources, Case};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn requests() -> Vec<RowInsert> {
    vec![RowInsert::new(vec![
        Value::Int32(226),
        Value::Binary(Vec::new()),
    ])]
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

// Closed109-byte TCNA4 literal, built before any matching producer call.
fn wire(case: &Case, id: i32) -> Vec<u8> {
    let mut result = Vec::from(*b"TCNA0004");
    result.extend_from_slice(&109_u32.to_le_bytes());
    result.extend_from_slice(&[4, 0, 0, 0]);
    result.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((case.before.len() as u64).to_le_bytes());
    hash.update(&case.before);
    result.extend_from_slice(&hash.finalize());
    for word in [13_u32, 1, 0] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.extend_from_slice(b"BinaryStorage");
    result.extend_from_slice(&28_u32.to_le_bytes());
    result.extend_from_slice(&2_u32.to_le_bytes());
    result.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0]);
    result.extend_from_slice(&id.to_le_bytes());
    result.extend_from_slice(&[7, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(result.len(), 109);
    result
}

fn intent(case: &Case, id: i32) -> NativeCompleteInsertIntent {
    NativeCompleteInsertIntent::decode(&wire(case, id), &mut budget(), &|| false).unwrap()
}

// Generic IO frames only frozen independent images, never native output goldens.
fn journal(case: &Case, after: &[u8], committed: bool) -> Storage {
    let mut target = Target::new(case.before.clone());
    let mut log = Storage::new(Vec::new());
    let prepared = prepare_resizable_expected(
        &mut target,
        &mut log,
        &case.before,
        after,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    if committed {
        prepared.commit(&mut budget(), &|| false).unwrap();
    } else {
        drop(prepared);
    }
    log
}

#[test]
#[ignore = "requires four root-split sources qualified before native producers"]
fn complete_root_splits_commit_tcna4_v2_and_confirm_without_writes_or_resize() {
    let cases = cases();
    let wires = cases.iter().map(|case| wire(case, 226)).collect::<Vec<_>>();
    for (case, expected_wire) in cases.iter().zip(wires) {
        assert_eq!(case.after.len(), (case.eof + 2) * 4096);
        let plan = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                "BinaryStorage",
                requests(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 1);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            expected_wire
        );
        let mut target = Target::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare_insert(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.storage.bytes.get_ref(), &case.after);
        assert_eq!(&log.bytes.get_ref()[..8], b"TCIMG002");
        let writes = target.storage.writes;
        let resizes = target.resizes;
        target.storage.fail_prefix = Some(0);
        let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_complete_insert_recovery(
            checked,
            intent(case, 226),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!((target.storage.writes, target.resizes), (writes, resizes));
    }
}

#[test]
#[ignore = "requires four literal root-split sources; journal framing is independent"]
fn two_index_tail_and_old_prefix_mixtures_rollback_and_forged_after_or_intent_stays_effect_free() {
    let mut recovered = 0;
    for case in cases() {
        let length = case.before.len();
        let mut states = Vec::new();
        for tail in [0, 1, 4096, 8191, 8192] {
            let mut bytes = case.before.clone();
            bytes.extend_from_slice(&case.after[length..length + tail]);
            states.push(bytes);
        }
        for page in [case.definition, case.root, case.data] {
            let mut bytes = case.before.clone();
            bytes[page * 4096..(page + 1) * 4096]
                .copy_from_slice(&case.after[page * 4096..(page + 1) * 4096]);
            assert_ne!(bytes, case.before);
            assert_ne!(bytes, case.after);
            states.push(bytes);
        }
        for initial in states {
            let mut target = Target::new(initial);
            let mut log = journal(&case, &case.after, false);
            let saved = log.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_complete_insert_recovery(
                    checked,
                    intent(&case, 226),
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
                assert_eq!(target.storage.bytes.get_ref(), &case.before);
            }
            let before = (target.storage.writes, target.resizes);
            target.storage.fail_prefix = Some(0);
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_complete_insert_recovery(
                checked,
                intent(&case, 226),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                RecoveryOutcome::RolledBack
            );
            assert_eq!((target.storage.writes, target.resizes), before);
            assert_eq!(log.bytes.get_ref(), &saved);
            recovered += 1;
        }
        let mut mixed = case.before.clone();
        mixed[case.root * 4096..(case.root + 1) * 4096]
            .copy_from_slice(&case.after[case.root * 4096..(case.root + 1) * 4096]);
        let mut target = Target::new(mixed.clone());
        let mut log = journal(&case, &case.after, true);
        let saved = log.bytes.get_ref().clone();
        let before = (target.storage.writes, target.resizes);
        let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_complete_insert_recovery(
            checked,
            intent(&case, 226),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            qualified.recover(&mut target, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        ));
        assert_eq!((target.storage.writes, target.resizes), before);
        assert_eq!(target.storage.bytes.get_ref(), &mixed);
        assert_eq!(log.bytes.get_ref(), &saved);

        let mut after = case.after.clone();
        after[case.root * 4096 + 489] ^= 1;
        for (forged, id) in [(&after, 226), (&case.after, 227)] {
            let target = Target::new(case.before.clone());
            let mut log = journal(&case, forged, false);
            let saved = log.bytes.get_ref().clone();
            let before = (target.storage.writes, target.resizes);
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_complete_insert_recovery(
                    checked,
                    intent(&case, id),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!((target.storage.writes, target.resizes), before);
            assert_eq!(target.storage.bytes.get_ref(), &case.before);
            assert_eq!(log.bytes.get_ref(), &saved);
        }
    }
    assert_eq!(recovered, 16);
}

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| assert_eq!(hash(raw), expected))
}

#[path = "root_split/resources.rs"]
mod resources;
