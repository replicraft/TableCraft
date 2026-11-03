//! SAME checked TCNI4/V1 reconstruction with causal DATA/INDEX/history mixtures.
use super::{Storage, all, budget, journals, scope, wire};
use journals::Marker;
use tablecraft_ace::{
    NativeRecoveryQualificationError, NativeUpdateIntent, qualify_native_update_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires held IgnoreNULL original/AFTER images"]
fn original_omission_intents_recover_every_changed_page_subset_without_counter_replay() {
    let mut contexts = 0;
    let mut committed_refusals = 0;
    let mut confirmations = 0;
    for case in all() {
        let pages = case.changed_pages();
        for mask in 0..1_usize << pages.len() {
            let mut original = case.before.clone();
            for (position, page) in pages.iter().enumerate() {
                if mask & (1 << position) != 0 {
                    let range = *page as usize * 4096..(*page as usize + 1) * 4096;
                    original[range.clone()].copy_from_slice(&case.after[range]);
                }
            }
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
                        NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
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
                        NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
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
                    confirmations += 1;
                }
                assert_eq!(log.writes, 0);
                assert_eq!(log.bytes.get_ref(), &saved);
                contexts += 1;
                committed_refusals += usize::from(mismatch);
            }
        }
    }
    assert_eq!((contexts, committed_refusals, confirmations), (114, 26, 88));
}

#[test]
#[ignore = "requires held IgnoreNULL original/AFTER images"]
fn checksum_valid_omission_forgeries_fail_fresh_native_replay_before_target_effects() {
    let mut refusals = 0;
    for case in all() {
        for offset in case.forged_offsets() {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut log = Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = log.bytes.get_ref().clone();
            let intent =
                NativeUpdateIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
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
    assert_eq!(refusals, 324);
}
