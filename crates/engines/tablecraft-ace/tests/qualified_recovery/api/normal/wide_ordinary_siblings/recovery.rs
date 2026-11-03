//! Same checked V1 intent replay for every subset of the three changed pages.
use super::{Storage, all, budget, journals, scope, wire};
use journals::Marker;
use tablecraft_ace::{
    NativeRecoveryQualificationError, NativeUpdateIntent, qualify_native_update_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires independently qualified wider-sibling source images"]
fn original_wide_requests_recover_all_page_subsets_repeat_and_confirm_without_replaying_effects() {
    let mut contexts = 0;
    let mut committed_mismatches = 0;
    for case in all() {
        let pages = [case.data, case.roots[1], case.roots[2]];
        let targets = if case.changed() == 0 { 1 } else { 8 };
        let mut seen = Vec::new();
        for mask in 0..targets {
            let mut original = case.before.clone();
            for (bit, page) in pages.into_iter().enumerate() {
                if mask & (1 << bit) != 0 {
                    let span = page * 4096..(page + 1) * 4096;
                    original[span.clone()].copy_from_slice(&case.after[span]);
                }
            }
            assert!(!seen.contains(&original));
            seen.push(original.clone());
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
                        assert_eq!(target.writes, 0);
                        assert_eq!(target.bytes.get_ref(), &original);
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
                }
                assert_eq!(log.writes, 0);
                assert_eq!(log.bytes.get_ref(), &saved);
                contexts += 1;
                committed_mismatches += usize::from(mismatch);
            }
        }
    }
    assert_eq!((contexts, committed_mismatches), (408, 112));
}

#[test]
#[ignore = "requires independently qualified wider-sibling source images"]
fn checksum_consistent_protected_companion_and_index_journals_fail_native_qualification() {
    let mut refusals = 0;
    for case in all() {
        for (offset, mask) in case.forgeries() {
            let mut after = case.after.clone();
            after[offset] ^= mask;
            let mut log = Storage::new(journals::journal(&case.before, &after, Marker::Pending));
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
    assert_eq!(refusals, 720);
}
