//! Independent intent replay protects NODE fences, aliases, histories and tails.
use super::{Storage, budget, cases, fixtures, intent, journals, scope};
use journals::Marker;
use tablecraft_ace::qualify_native_delete_recovery;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires eight independent native/SDK-qualified repeated retirement source images"]
fn full_and_internal_parent_only_targets_recover_or_refuse_without_replacing_the_journal() {
    let mut contexts = 0;
    for case in cases() {
        let mut mixed = case.before.clone();
        let start = case.parent * 4096;
        mixed[start..start + 4096].copy_from_slice(&case.after[start..start + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);

        let mut data_only = case.before.clone();
        let count = case.definition * 4096 + 16;
        data_only[count..count + 4].copy_from_slice(&case.after[count..count + 4]);
        for ordinal in case.recipe.selected() {
            let offset = case.words[ordinal];
            data_only[offset..offset + 2].copy_from_slice(&case.after[offset..offset + 2]);
        }
        assert_ne!(data_only, case.before);
        assert_ne!(data_only, case.after);

        for initial in [&case.after, &mixed, &data_only] {
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mismatch = committed && initial != &case.after;
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let expected = if mismatch {
                    initial
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
    assert_eq!(contexts, 72);
}

#[test]
#[ignore = "requires eight independent native/SDK-qualified repeated retirement source images"]
fn forged_retirement_maps_headers_full_fences_rows_and_complements_cannot_authorize_recovery() {
    let mut refused = 0;
    for case in cases() {
        for offset in fixtures::forged_offsets(&case) {
            let mut after = case.after.clone();
            after[offset] ^= 1;
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let mut journal = Storage::new(journals::journal(&case.before, &after, marker));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                assert!(matches!(
                    qualify_native_delete_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(tablecraft_ace::NativeRecoveryQualificationError::ImageMismatch)
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 456);
}

#[test]
#[ignore = "requires qualified chain4 and original repeat8 views"]
fn older_semantic_empty_targets_cannot_authorize_native_chain_recovery() {
    let mut refused = 0;
    for case in cases() {
        let Some(wrong) = &case.wrong_history_after else {
            continue;
        };
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let mut journal = Storage::new(journals::journal(&case.before, wrong, marker));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_delete_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(tablecraft_ace::NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            refused += 1;
        }
    }
    assert_eq!(refused, 6);
}
