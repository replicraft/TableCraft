//! Independent intent replay protects NODE fences, aliases, histories and tails.
use super::{Storage, budget, cases, fixtures, intent, journals, scope};
use journals::Marker;
use tablecraft_ace::{
    EditError, NativeRecoveryQualificationError, RecoverableEditError, RowDelete,
    qualify_native_delete_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires four independent native/SDK-qualified retained NODE source images"]
fn full_and_root_only_targets_recover_or_refuse_without_replacing_the_journal() {
    let mut contexts = 0;
    for case in cases() {
        let mut mixed = case.before.clone();
        let start = case.roots[0] * 4096;
        mixed[start..start + 4096].copy_from_slice(&case.after[start..start + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);

        for initial in [&case.after, &mixed] {
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mismatch = committed && initial == &mixed;
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let expected = if mismatch {
                    &mixed
                } else if committed {
                    &case.after
                } else {
                    &case.before
                };
                let mut target = Storage::new(initial.clone());
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.after, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_delete_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let recovered = qualified.recover(&mut target, &mut budget(), &|| false);
                    if mismatch {
                        assert!(matches!(recovered, Err(JournalError::TargetMismatch)));
                    } else {
                        assert_eq!(recovered.unwrap(), outcome);
                    }
                    assert_eq!(target.bytes.get_ref(), expected);
                }
                if committed {
                    assert_eq!(target.writes, 0);
                }
                let writes = target.writes;
                target.fail_prefix = Some(0);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_delete_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let confirmed = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if mismatch {
                    assert!(matches!(confirmed, Err(JournalError::TargetMismatch)));
                } else {
                    assert_eq!(confirmed.unwrap(), outcome);
                }
                assert_eq!(target.bytes.get_ref(), expected);
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 12);
}

#[test]
#[ignore = "requires four independent native/SDK-qualified retained NODE source images"]
fn stale_parent_fences_and_inactive_leaf_tails_cannot_authorize_recovery() {
    let mut refusals = 0;
    for case in cases() {
        let mut forgeries = Vec::new();
        for root in case.roots {
            let mut stale = case.after.clone();
            let start = root * 4096 + 480;
            stale[start..start + 13].copy_from_slice(&case.before[start..start + 13]);
            assert_ne!(stale, case.after);
            forgeries.push(stale);
        }
        for offset in [
            case.left[0] * 4096 + 480 + 301 * 9,
            case.left[1] * 4096 + 480 + 393 * 9,
            case.definition * 4096 + 67,
            case.definition * 4096 + 79,
        ] {
            let mut altered = case.after.clone();
            altered[offset] ^= 1;
            forgeries.push(altered);
        }
        let mut missing_guard = case.after.clone();
        let start = case.left[1] * 4096;
        missing_guard[start..start + 4096].copy_from_slice(&case.before[start..start + 4096]);
        forgeries.push(missing_guard);

        for forged in forgeries {
            assert_ne!(forged, case.after);
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let mut journal = Storage::new(journals::journal(&case.before, &forged, marker));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                assert!(matches!(
                    qualify_native_delete_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    ),
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                refusals += 1;
            }
        }
        // No semantic equivalence can replace an exact original raw witness.
        let mut stale = fixtures::record(301);
        stale[6] ^= 1;
        assert_ne!(stale, fixtures::record(301));
        let error = super::image(&case.before)
            .plan_recoverable_ordinary_row_deletes(
                "Items",
                vec![RowDelete::new(301, stale)],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RecoverableEditError::Edit(EditError::Conflict)
        ));
    }
    assert_eq!(refusals, 42);
}
