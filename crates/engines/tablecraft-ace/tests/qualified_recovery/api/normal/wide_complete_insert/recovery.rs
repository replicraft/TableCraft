//! Independent generic journal framing over literal images, retaining one token.
use super::{Storage, Target, budget, fixtures, scope};
use fixtures::View;
use tablecraft_ace::{NativeRecoveryQualificationError, qualify_native_complete_insert_recovery};
use tablecraft_io::journal::{
    JournalError, RecoveryOutcome, inspect_image, prepare_expected, prepare_resizable_expected,
};

fn journal(view: &View, after: &[u8], marker: usize) -> Storage {
    let mut target = Target::new(view.source.before.clone());
    let mut journal = Storage::new(Vec::new());
    if after.len() == view.source.before.len() {
        let prepared = prepare_expected(
            &mut target,
            &mut journal,
            &view.source.before,
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
    } else {
        let prepared = prepare_resizable_expected(
            &mut target,
            &mut journal,
            &view.source.before,
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
    }
    if marker == 1 {
        journal.bytes.get_mut().extend_from_slice(b"COM");
    }
    journal
}

#[test]
fn tcna4_recovery_replays_both_framings_and_committed_mixed_targets_never_write() {
    let mut contexts = 0;
    for view in fixtures::cases() {
        for marker in [0, 1, 2] {
            let mut mixed = view.source.before.clone();
            mixed[7 * 4096..8 * 4096].copy_from_slice(&view.source.after[7 * 4096..8 * 4096]);
            assert_ne!(mixed, view.source.before);
            assert_ne!(mixed, view.source.after);
            for initial in [view.source.after.clone(), mixed.clone()] {
                let refuse = marker == 2 && initial == mixed;
                let expected = if marker == 2 {
                    &view.source.after
                } else {
                    &view.source.before
                };
                let mut target = Target::new(initial.clone());
                let mut log = journal(&view, &view.source.after, marker);
                let saved = log.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_complete_insert_recovery(
                        checked,
                        fixtures::intent(&view),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let before = (target.storage.writes, target.resizes);
                    let result = qualified.recover(&mut target, &mut budget(), &|| false);
                    if refuse {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.storage.bytes.get_ref(), &initial);
                        assert_eq!((target.storage.writes, target.resizes), before);
                    } else {
                        assert_eq!(
                            result.unwrap(),
                            if marker == 2 {
                                RecoveryOutcome::Committed
                            } else {
                                RecoveryOutcome::RolledBack
                            }
                        );
                        assert_eq!(target.storage.bytes.get_ref(), expected);
                    }
                }
                let before = (target.storage.writes, target.resizes);
                target.storage.fail_prefix = Some(0);
                let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_complete_insert_recovery(
                    checked,
                    fixtures::intent(&view),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let result = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if refuse {
                    assert!(matches!(result, Err(JournalError::TargetMismatch)));
                } else {
                    assert_eq!(
                        result.unwrap(),
                        if marker == 2 {
                            RecoveryOutcome::Committed
                        } else {
                            RecoveryOutcome::RolledBack
                        }
                    );
                }
                assert_eq!((target.storage.writes, target.resizes), before);
                assert_eq!(log.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 96);
}

#[test]
fn full_replay_refuses_protected_source_and_old_record_payload_forgeries() {
    let mut refused = 0;
    for view in fixtures::cases() {
        for at in [24, 4 * 4096 + 15, 8 * 4096 + 4095] {
            let mut forged = view.source.after.clone();
            forged[at] ^= 1;
            let mut log = journal(&view, &forged, 0);
            let writes = log.writes;
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_complete_insert_recovery(
                    checked,
                    fixtures::intent(&view),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(log.writes, writes);
            refused += 1;
        }
    }
    assert_eq!(refused, 48);
}
