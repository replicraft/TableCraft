//! Causal Guard-leaf publication, complete raw witnesses and nullable identity proof.
use super::{Storage, budget, fixtures, journals, scope};
use journals::Marker;
use tablecraft_ace::{
    EditError, NativeDeleteIntent, NativeRecoveryQualificationError, qualify_native_delete_recovery,
};
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
#[ignore = "requires native source16 and independent SDK80 DESC/NULL qualification"]
fn descending_nullable_delete_replays_full_and_guard_only_causal_publication() {
    let mut contexts = 0;
    for case in fixtures::cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            replay(&case, marker, case.after.clone());
            contexts += 1;
        }
        let mut mixed = case.before.clone();
        let base = case.roots[1] * 4096;
        mixed[base..base + 4096].copy_from_slice(&case.after[base..base + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for marker in [Marker::Pending, Marker::Partial] {
            replay(&case, marker, mixed.clone());
            contexts += 1;
        }
        let mut target = Storage::new(mixed.clone());
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Committed,
        ));
        let saved = journal.bytes.get_ref().clone();
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
        assert_eq!(target.bytes.get_ref(), &mixed);
        assert_eq!(target.writes, 0);
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);
    }
    assert_eq!(contexts, 40);
}

fn reject_after(case: &fixtures::Case, forged: Vec<u8>) {
    assert_ne!(forged, case.after);
    let mut journal = Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
    let saved = journal.bytes.get_ref().clone();
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_delete_recovery(
            checked,
            fixtures::intent(case),
            scope(),
            &mut budget(),
            &|| false,
        ),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(journal.writes, 0);
    assert_eq!(journal.bytes.get_ref(), &saved);
}

#[test]
#[ignore = "requires native source16 and independent SDK80 DESC/NULL qualification"]
fn logical_null_never_authorizes_inactive_payload_tail_or_missing_member_forgery() {
    let mut after_refusals = 0;
    let mut original_refusals = 0;
    for case in fixtures::cases() {
        for offset in [
            case.data * 4096 + 4091, // Row0 NULL Guard inactive payload, even if tombstoned.
            case.data * 4096 + 4095, // Original NULL presence mask remains exact.
            case.roots[1] * 4096 + 502, // Last byte of original NULL entry is always vacated.
            case.definition * 4096 + 79, // Guard history never decreases with live membership.
        ] {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            reject_after(&case, forged);
            after_refusals += 1;
        }
        let mut forged = case.after.clone();
        let base = case.roots[1] * 4096;
        forged[base..base + 4096].copy_from_slice(&case.before[base..base + 4096]);
        reject_after(&case, forged);
        after_refusals += 1;
        if case.recipe == "present-middle" {
            // Healthy literal AFTER retains Guard2 / row2 (9B) and NULL / row0 (5B).
            // Removing only the NULL member cannot satisfy the unchanged live row0.
            assert_eq!(&case.after[base + 2..base + 4], &3602_u16.to_le_bytes());
            assert_eq!(case.after[base + 489], 255);
            let mut forged = case.after.clone();
            forged[base + 2..base + 4].copy_from_slice(&3607_u16.to_le_bytes());
            forged[base + 28] &= !(1 << 6); // Clear end14, retain end9.
            reject_after(&case, forged);
            after_refusals += 1;
            let mut forged = case.after.clone();
            forged[base + 489] = 0; // ASC NULL marker does not implement DESC NULL.
            reject_after(&case, forged);
            after_refusals += 1;
        }
        if case.recipe == "null-first" {
            let mut wire = fixtures::wire(&case, false);
            assert_eq!(&wire[95..99], &[0; 4]);
            wire[95] = 1; // Original raw NULL witness changed, despite identical logical NULL.
            let intent = NativeDeleteIntent::decode(&wire, &mut budget(), &|| false).unwrap();
            let mut journal = Storage::new(journals::journal(
                &case.before,
                &case.after,
                Marker::Pending,
            ));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false,),
                Err(NativeRecoveryQualificationError::Edit(EditError::Conflict))
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            original_refusals += 1;
        }
    }
    assert_eq!((after_refusals, original_refusals), (44, 2));
}
