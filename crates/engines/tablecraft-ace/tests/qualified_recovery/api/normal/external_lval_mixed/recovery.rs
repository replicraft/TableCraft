//! SAME checked V1 tokens replay original groups; committed mixtures never repair.
use super::{Storage, budget, cases, fixture, journals, scope, wire};
use journals::Marker;
use tablecraft_ace::{
    NativeRecoveryQualificationError, NativeUpdateIntent, qualify_native_update_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn external_mixed_recovery_causal_components_repeat_and_confirm_without_repairing_committed_mixtures()
 {
    let mut contexts = 0;
    let mut committed_refusals = 0;
    for case in cases() {
        let mut targets = vec![case.after.clone()];
        targets.extend(fixture::partial_targets(&case));
        for reverse in [false, true] {
            let original_wire = wire(&case, reverse);
            for initial in &targets {
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
                    target.fail_sync = mismatch;
                    journal.fail_sync = mismatch;
                    for _ in 0..2 {
                        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                        let intent =
                            NativeUpdateIntent::decode(&original_wire, &mut budget(), &|| false)
                                .unwrap();
                        let qualified = qualify_native_update_recovery(
                            checked,
                            intent,
                            scope(),
                            &mut budget(),
                            &|| false,
                        )
                        .unwrap();
                        let result = qualified.recover(&mut target, &mut budget(), &|| false);
                        if mismatch {
                            assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        } else {
                            assert_eq!(result.unwrap(), outcome);
                        }
                        assert_eq!(target.bytes.get_ref(), expected);
                    }
                    if committed {
                        assert_eq!(target.writes, 0);
                    }
                    let writes = target.writes;
                    target.fail_prefix = Some(0);
                    target.fail_sync = mismatch;
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let intent =
                        NativeUpdateIntent::decode(&original_wire, &mut budget(), &|| false)
                            .unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let result = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                    if mismatch {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                    } else {
                        assert_eq!(result.unwrap(), outcome);
                    }
                    assert_eq!(target.writes, writes);
                    if mismatch {
                        assert!(target.fail_sync);
                        assert!(journal.fail_sync);
                    }
                    assert_eq!(journal.writes, 0);
                    assert_eq!(journal.bytes.get_ref(), &saved);
                    committed_refusals += usize::from(mismatch);
                    contexts += 1;
                }
            }
        }
    }
    assert_eq!((contexts, committed_refusals), (480, 128));
}

#[test]
#[ignore = "requires independently qualified external mixed20 source"]
fn external_mixed_protected_bytes_cannot_be_self_consistent_journal_authority() {
    let mut refused = 0;
    for case in cases() {
        for reverse in [false, true] {
            let original_wire = wire(&case, reverse);
            for at in fixture::forged_offsets(&case) {
                let mut forged = case.after.clone();
                forged[at] ^= 1;
                let mut journal =
                    Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent =
                    NativeUpdateIntent::decode(&original_wire, &mut budget(), &|| false).unwrap();
                assert!(matches!(
                    qualify_native_update_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 576);
}
