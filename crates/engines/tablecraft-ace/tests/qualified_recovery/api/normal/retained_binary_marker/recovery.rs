//! Original TCNI4 replay consumes the same checked V1 token before effects.
use super::{Storage, all, budget, journals, scope, wire};
use journals::Marker;
use tablecraft_ace::{
    NativeRecoveryQualificationError, NativeUpdateIntent, qualify_native_update_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires six independently qualified retained Binary Marker images"]
fn marker_original_intents_recover_repeat_and_confirm_including_causal_mixed_targets() {
    let mut contexts = 0;
    let mut committed_mixed = 0;
    for case in all() {
        let mut targets = vec![case.before.clone()];
        if case.before != case.after {
            targets.push(case.after.clone());
            let differences: Vec<_> = case
                .before
                .iter()
                .zip(&case.after)
                .enumerate()
                .filter_map(|(offset, (old, new))| (old != new).then_some(offset))
                .collect();
            if differences.len() > 1 {
                // Batch writes two independent Marker cells in one DATA page.
                // First-cell-only is causal and differs from BOTH complete images.
                let mut partial = case.before.clone();
                partial[differences[0]] = case.after[differences[0]];
                assert_ne!(partial, case.before);
                assert_ne!(partial, case.after);
                targets.push(partial);
            }
        }
        for original in targets {
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mismatch = committed && original != case.after;
                let expected = if committed { &case.after } else { &case.before };
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let mut target = Storage::new(original.clone());
                let mut log = Storage::new(journals::journal(&case.before, &case.after, marker));
                let saved = log.bytes.get_ref().clone();
                target.fail_prefix = mismatch.then_some(0);
                target.fail_sync = mismatch;
                log.fail_sync = mismatch;
                for _ in 0..2 {
                    let intent =
                        NativeUpdateIntent::decode(&wire(&case, false), &mut budget(), &|| false)
                            .unwrap();
                    let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
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
                        assert_eq!(target.bytes.get_ref(), &original);
                        assert_eq!(target.writes, 0);
                        assert_eq!(target.fail_prefix, Some(0));
                        assert!(target.fail_sync && log.fail_sync);
                    } else {
                        assert_eq!(result.unwrap(), outcome);
                        assert_eq!(target.bytes.get_ref(), expected);
                    }
                }
                if !mismatch {
                    let writes = target.writes;
                    target.fail_prefix = Some(0);
                    let intent =
                        NativeUpdateIntent::decode(&wire(&case, false), &mut budget(), &|| false)
                            .unwrap();
                    let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_update_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        qualified
                            .confirm_completed(&mut target, &mut budget(), &|| false)
                            .unwrap(),
                        outcome
                    );
                    assert_eq!(target.writes, writes);
                    assert_eq!(target.bytes.get_ref(), expected);
                }
                assert_eq!(log.writes, 0);
                assert_eq!(log.bytes.get_ref(), &saved);
                contexts += 1;
                committed_mixed += usize::from(mismatch);
            }
        }
    }
    // Per header: batch3 +NULL2 +noop1 +empty1 targets, each3 markers.
    assert_eq!(contexts, 42);
    assert_eq!(committed_mixed, 6);
}

#[test]
#[ignore = "requires six independently qualified retained Binary Marker images"]
fn marker_native_replay_rejects_self_consistent_protected_byte_journals_without_effects() {
    let mut refusals = 0;
    for case in all() {
        for offset in case.forged_offsets() {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut log = Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = log.bytes.get_ref().clone();
            let intent =
                NativeUpdateIntent::decode(&wire(&case, false), &mut budget(), &|| false).unwrap();
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(log.writes, 0);
            assert_eq!(log.bytes.get_ref(), &saved);
            refusals += 1;
        }
    }
    assert_eq!(refusals, 176);
}
