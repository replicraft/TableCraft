//! Same checked V1 original replay over genuine mixed DATA/INDEX targets.
use super::super::model as journals;
use super::{Case, NativeUpdateIntent, Storage, all, budget, scope};
use journals::Marker;
use tablecraft_ace::{NativeRecoveryQualificationError, qualify_native_update_recovery};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

fn mixed(case: &Case) -> Vec<u8> {
    let mut target = case.before.clone();
    // The first changed byte belongs to a complete original-relative patch
    // image; leave every other changed byte at BEFORE. This is an actual mixed
    // target rather than a full AFTER mislabeled as interrupted.
    let changed: Vec<_> = case
        .before
        .iter()
        .zip(&case.after)
        .enumerate()
        .filter_map(|(offset, (before, after))| (before != after).then_some(offset))
        .collect();
    assert!(changed.len() > 1);
    target[changed[0]] = case.after[changed[0]];
    assert_ne!(target, case.before);
    assert_ne!(target, case.after);
    target
}

#[test]
#[ignore = "requires independently qualified wider UPDATE literal images"]
fn wider_updates_recover_pending_mixed_and_refuse_committed_mixed_without_effects() {
    let cases = all();
    for case in cases.iter().filter(|case| case.before != case.after) {
        for (committed, confirm) in [(false, false), (true, false), (true, true)] {
            let marker = if committed {
                Marker::Committed
            } else {
                Marker::Pending
            };
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let saved_journal = journal.bytes.get_ref().clone();
            let mut target = Storage::new(mixed(case));
            let saved_target = target.bytes.get_ref().clone();
            let original =
                NativeUpdateIntent::decode(&case.wire, &mut budget(), &|| false).unwrap();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified =
                qualify_native_update_recovery(checked, original, scope(), &mut budget(), &|| {
                    false
                })
                .unwrap();
            let outcome = if confirm {
                qualified.confirm_completed(&mut target, &mut budget(), &|| false)
            } else {
                qualified.recover(&mut target, &mut budget(), &|| false)
            };
            if committed {
                assert!(matches!(outcome, Err(JournalError::TargetMismatch)));
                assert_eq!(target.writes, 0);
                assert_eq!(target.bytes.get_ref(), &saved_target);
            } else {
                assert_eq!(outcome.unwrap(), RecoveryOutcome::RolledBack);
                assert_eq!(target.bytes.get_ref(), &case.before);
            }
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved_journal);
        }
    }
}

#[test]
#[ignore = "requires independently qualified wider UPDATE literal images"]
fn wider_original_replay_rejects_final_image_corruption() {
    let cases = all();
    for case in cases
        .iter()
        .filter(|case| matches!(case.recipe, "joint-marker" | "primary-swap-extremes"))
    {
        // Source-derived TD histories, DATA/masks, each physical ROW/bitmap and
        // absolute inactive tail all remain bound to the fresh original intent.
        for offset in super::source::forged_offsets(case) {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let original =
                NativeUpdateIntent::decode(&case.wire, &mut budget(), &|| false).unwrap();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(checked, original, scope(), &mut budget(), &|| {
                    false
                }),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}
