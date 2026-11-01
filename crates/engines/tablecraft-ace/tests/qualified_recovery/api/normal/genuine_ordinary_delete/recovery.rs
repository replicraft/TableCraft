//! Same-session TCND recovery over real partial TD/DATA publication states.
use super::{Storage, budget, fixtures, journals, scope};
use journals::Marker;
use tablecraft_ace::qualify_native_delete_recovery;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

fn replay(case: &fixtures::Case, marker: Marker, initial: Vec<u8>) {
    let committed = matches!(marker, Marker::Committed);
    let outcome = if committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    };
    let expected = if committed { &case.after } else { &case.before };
    let mut target = Storage::new(initial);
    let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
    let saved = journal.bytes.get_ref().clone();
    for _ in 0..2 {
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_delete_recovery(
            checked,
            fixtures::intent(case),
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
        fixtures::intent(case),
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

#[test]
#[ignore = "requires all four independently qualified ordinary DELETE images"]
fn original_delete_intent_replays_settled_and_truly_partial_images_without_replacing_journal() {
    for case in fixtures::cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            replay(&case, marker, case.after.clone());
        }
        for page in [case.definition, case.data] {
            let mut mixed = case.before.clone();
            mixed[page * 4096..(page + 1) * 4096]
                .copy_from_slice(&case.after[page * 4096..(page + 1) * 4096]);
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            for marker in [Marker::Pending, Marker::Partial] {
                replay(&case, marker, mixed.clone());
            }
            let mut target = Storage::new(mixed.clone());
            let mut journal = Storage::new(journals::journal(
                &case.before,
                &case.after,
                Marker::Committed,
            ));
            let saved = journal.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_delete_recovery(
                    checked,
                    fixtures::intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert!(matches!(
                    qualified.recover(&mut target, &mut budget(), &|| false),
                    Err(JournalError::TargetMismatch)
                ));
            }
            assert_eq!(target.bytes.get_ref(), &mixed);
            assert_eq!(target.writes, 0);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

#[test]
#[ignore = "requires all four independently qualified ordinary DELETE images"]
fn self_consistent_journal_digests_do_not_authorize_protected_after_bytes() {
    for case in fixtures::cases() {
        // Header, retained deleted payload, TD metadata, map storage, and an
        // unrelated system page are individually outside the two allowed laws.
        let offsets = [
            152,
            case.data * 4096 + 4095,
            case.definition * 4096 + 20,
            4096 + 4027,
            17 * 4096 + 100,
        ];
        for offset in offsets {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(
                qualify_native_delete_recovery(
                    checked,
                    fixtures::intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
        // A stale witness remains syntactically legal TCND data. Recomputing
        // its framing must not let it substitute a different original record.
        let mut stale = fixtures::wire(&case);
        stale[88 + 2] ^= 1;
        let intent =
            tablecraft_ace::NativeDeleteIntent::decode(&stale, &mut budget(), &|| false).unwrap();
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(
            qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false)
                .is_err()
        );
        assert_eq!(journal.writes, 0);
    }
}
