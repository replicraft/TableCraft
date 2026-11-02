//! Cold replay derives DATE from the same original request and immutable journal.
use super::super::{Storage, budget, model as journals, scope};
use super::{fixtures, sources, wire};
use journals::Marker;
use tablecraft_ace::{
    NativeOmittedInsertIntent, NativeRecoveryQualificationError,
    qualify_native_omitted_insert_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires source36 native parser/bit/Source gates before matching producers"]
fn date_defaults_replay_same_checked_v1_and_committed_mixed_targets_refuse() {
    let sources = sources();
    let case = fixtures::CASES[5];
    for header in [5, 6] {
        let before = sources.bytes(header, case, "before");
        let after = sources.bytes(header, case, "omitted");
        for marker in [Marker::Pending, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let outcome = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let settled = if committed { &after } else { &before };
            let mut target = Storage::new(after.clone());
            let mut journal = Storage::new(journals::journal(&before, &after, marker));
            let saved = journal.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent = NativeOmittedInsertIntent::decode(
                    &wire(&before, Some("omitted")),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let qualified = qualify_native_omitted_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    outcome
                );
                assert_eq!(target.bytes.get_ref(), settled);
            }
            let writes = target.writes;
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent = NativeOmittedInsertIntent::decode(
                &wire(&before, Some("omitted")),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let qualified = qualify_native_omitted_insert_recovery(
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
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
        let mut mixed = before.clone();
        mixed[7 * 4096..8 * 4096].copy_from_slice(&after[7 * 4096..8 * 4096]);
        assert_ne!(mixed, before);
        assert_ne!(mixed, after);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let mut target = Storage::new(mixed.clone());
            let mut journal = Storage::new(journals::journal(&before, &after, marker));
            let saved = journal.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent = NativeOmittedInsertIntent::decode(
                    &wire(&before, Some("omitted")),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let qualified = qualify_native_omitted_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let result = qualified.recover(&mut target, &mut budget(), &|| false);
                if committed {
                    assert!(matches!(result, Err(JournalError::TargetMismatch)));
                    assert_eq!(target.bytes.get_ref(), &mixed);
                    assert_eq!(target.writes, 0);
                } else {
                    assert_eq!(result.unwrap(), RecoveryOutcome::RolledBack);
                    assert_eq!(target.bytes.get_ref(), &before);
                }
            }
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
        // The frame and digest are self-consistent, but native declaration
        // replay cannot produce this valid changed DATE low bit.
        let mut forged = after.clone();
        forged[8 * 4096 + 4066 + 6] ^= 1;
        let mut journal = Storage::new(journals::journal(&before, &forged, Marker::Pending));
        let saved = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = NativeOmittedInsertIntent::decode(
            &wire(&before, Some("omitted")),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            qualify_native_omitted_insert_recovery(
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
    }
}
