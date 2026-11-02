//! Source-first original-complete-row TCNA3 and sealed V2 recovery effects.
use super::{Storage, budget, resizable_storage::Target, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeEofInsertIntent, NativeImage, NativeInsertIntent, NativeRecoveryQualificationError,
    RecoverableNativeInsertOperation, RowInsert, qualify_native_eof_insert_recovery, read_table,
};
use tablecraft_io::journal::{
    JournalError, RecoveryOutcome, inspect_v2, prepare_resizable_expected,
};
use tablecraft_testkit::fixtures::access::genuine_eof_insert::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
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

fn requests(case: &Case) -> Vec<RowInsert> {
    sources::requests(case.batch)
        .into_iter()
        .map(RowInsert::new)
        .collect()
}

fn wire(case: &Case) -> Vec<u8> {
    let rows = if case.batch { 255 } else { 1 };
    let length = 76 + 36 * rows;
    let mut wire = Vec::from(*b"TCNA0003");
    wire.extend_from_slice(&(length as u32).to_le_bytes());
    wire.extend_from_slice(&[3, 0, 0, 0]);
    wire.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((case.before.len() as u64).to_le_bytes());
    hash.update(&case.before);
    wire.extend_from_slice(&hash.finalize());
    for number in [8_u32, rows as u32, 0] {
        wire.extend_from_slice(&number.to_le_bytes());
    }
    wire.extend_from_slice(b"Sentinel");
    let ids = if case.batch { 24..=278_i32 } else { 260..=260 };
    for id in ids {
        wire.extend_from_slice(&36_u32.to_le_bytes());
        wire.extend_from_slice(&3_u32.to_le_bytes());
        wire.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0]);
        wire.extend_from_slice(&id.to_le_bytes());
        wire.extend_from_slice(&[0; 16]);
    }
    assert_eq!(wire.len(), length);
    wire
}

fn intent(case: &Case) -> NativeEofInsertIntent {
    NativeEofInsertIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

// Generic IO framing consumes only independent source images, never native output.
fn journal(case: &Case, after: &[u8], committed: bool) -> Storage {
    let mut discarded = Target::new(case.before.clone());
    let mut journal = Storage::new(Vec::new());
    let prepared = prepare_resizable_expected(
        &mut discarded,
        &mut journal,
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
        journal.bytes.get_mut().extend_from_slice(b"COM");
    }
    journal
}

#[test]
#[ignore = "requires eight independent source native/SDK views for complete-row EOF INSERT"]
fn public_auto_and_tcna3_commit_exact_batch_single_images_and_original_rows() {
    for case in cases() {
        let original_wire = wire(&case);
        assert!(NativeInsertIntent::decode(&original_wire, &mut budget(), &|| false).is_err());
        let plan = image(&case.before)
            .plan_recoverable_ordinary_auto_row_inserts(
                "Sentinel",
                requests(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let RecoverableNativeInsertOperation::AllocateEof(plan) = plan else {
            panic!("real metadata capacity requires represented EOF");
        };
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), if case.batch { 255 } else { 1 });
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            original_wire
        );
        assert_eq!(intent(&case).rows().len(), plan.inserted_rows());
        for (actual, expected) in plan
            .intent()
            .rows()
            .iter()
            .zip(sources::requests(case.batch))
        {
            assert_eq!(actual.values(), expected);
        }
        let mut target = Target::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare_insert(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            if case.batch { 255 } else { 1 }
        );
        assert_eq!(target.storage.bytes.get_ref(), &case.after);
        assert_eq!(&journal.bytes.get_ref()[..8], b"TCIMG002");
        let rows = read_table(
            &mut Cursor::new(target.storage.bytes.get_ref()),
            scope(),
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows(), sources::rows(&case, true));
        let writes = target.storage.writes;
        let resizes = target.resizes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_eof_insert_recovery(
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
            RecoveryOutcome::Committed
        );
        assert_eq!((target.storage.writes, target.resizes), (writes, resizes));
    }
}

#[test]
#[ignore = "requires eight independent source native/SDK views for complete-row EOF INSERT"]
fn pending_partial_eof_and_mixed_metadata_targets_rollback_but_committed_mismatch_has_no_effects() {
    let mut contexts = 0;
    for case in cases() {
        let length = case.before.len();
        let mut states = Vec::new();
        for tail in [0, 1, 14, 4095, 4096] {
            let mut raw = case.before.clone();
            raw.extend_from_slice(&case.after[length..length + tail]);
            states.push(raw);
        }
        let mut maps_only = case.after.clone();
        let td = case.definition * 4096;
        maps_only[td..td + 4096].copy_from_slice(&case.before[td..td + 4096]);
        states.push(maps_only);
        for current in states {
            assert_ne!(current, case.after);
            for committed in [false, true] {
                let mut target = Target::new(current.clone());
                let mut journal = journal(&case, &case.after, committed);
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_eof_insert_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let writes = target.storage.writes;
                    let resizes = target.resizes;
                    let result = qualified.recover(&mut target, &mut budget(), &|| false);
                    if committed {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.storage.bytes.get_ref(), &current);
                        assert_eq!((target.storage.writes, target.resizes), (writes, resizes));
                    } else {
                        assert_eq!(result.unwrap(), RecoveryOutcome::RolledBack);
                        assert_eq!(target.storage.bytes.get_ref(), &case.before);
                    }
                }
                let writes = target.storage.writes;
                let resizes = target.resizes;
                let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_eof_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let result = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if committed {
                    assert!(matches!(result, Err(JournalError::TargetMismatch)));
                } else {
                    assert_eq!(result.unwrap(), RecoveryOutcome::RolledBack);
                }
                assert_eq!((target.storage.writes, target.resizes), (writes, resizes));
                assert_eq!(journal.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 48);
}

#[test]
#[ignore = "requires eight independent source native/SDK views for complete-row EOF INSERT"]
fn fresh_tcna3_qualification_rejects_role_record_mask_count_and_readonly_ole_forgeries() {
    let mut refused = 0;
    for case in cases() {
        for at in [
            4096 + 4032 + case.eof / 8,
            case.bitmap * 4096 + 4 + case.eof / 8,
            case.maps * 4096 + 3963 + case.eof / 8,
            case.eof * 4096 + 14,
            case.eof * 4096 + 4095,
            case.definition * 4096 + 16,
            case.root * 4096 + 4095,
        ] {
            let mut changed = case.after.clone();
            changed[at] ^= 1;
            let mut journal = journal(&case, &changed, false);
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_eof_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.bytes.get_ref(), &saved);
            refused += 1;
        }
    }
    assert_eq!(refused, 28);
}
