//! Original TCNA batch replay protects written row order and all unaffected content.
use super::{Storage, budget, fixtures, journals, scope};
use journals::Marker;
use tablecraft_ace::{
    NativeInsertIntent, NativeRecoveryQualificationError, qualify_native_insert_recovery,
};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};

fn replay(case: &fixtures::Case, marker: Marker, initial: Vec<u8>) {
    let outcome = if matches!(marker, Marker::Committed) {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    };
    let expected = if matches!(outcome, RecoveryOutcome::Committed) {
        &case.after
    } else {
        &case.before
    };
    let mut target = Storage::new(initial);
    let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
    let saved = journal.bytes.get_ref().clone();
    for _ in 0..2 {
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_insert_recovery(
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
    let writes = target.writes;
    target.fail_prefix = Some(0);
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_native_insert_recovery(
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
        outcome,
    );
    assert_eq!(target.writes, writes);
    assert_eq!(journal.writes, 0);
    assert_eq!(journal.bytes.get_ref(), &saved);
}

#[test]
#[ignore = "requires all independently qualified single-row and multirow INSERT source images"]
fn multirow_original_intents_recover_full_and_causal_partial_publication() {
    for case in fixtures::cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            replay(&case, marker, case.after.clone());
        }
        for page in [case.definition as usize, case.data as usize] {
            let mut mixed = case.before.clone();
            mixed[page * 4096..(page + 1) * 4096]
                .copy_from_slice(&case.after[page * 4096..(page + 1) * 4096]);
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            for marker in [Marker::Pending, Marker::Partial] {
                replay(&case, marker, mixed.clone());
            }
        }
        // Authored interruption: TD and both directory entries are replacement
        // bytes, but the second inserted payload remains its original free
        // extent. This is a recovery fixture, not a healthy third source image.
        let mut one_record = case.after.clone();
        let base = case.data as usize * 4096;
        let second_end = if case.reverse { 4007 } else { 4009 };
        one_record[base + 3986..base + second_end]
            .copy_from_slice(&case.before[base + 3986..base + second_end]);
        assert_ne!(one_record, case.before);
        assert_ne!(one_record, case.after);
        for marker in [Marker::Pending, Marker::Partial] {
            replay(&case, marker, one_record.clone());
        }
    }
}

#[test]
#[ignore = "requires all independently qualified single-row and multirow INSERT source images"]
fn multirow_batch_order_and_protected_records_are_required_before_recovery_effects() {
    for case in fixtures::cases() {
        let data = case.data as usize * 4096;
        for offset in [
            data + 4095, // Original OLE/presence byte, never a supplied insertion cell.
            data + 4030, // Original row field count.
            data + 3992, // Source-literal new record payload.
            data + 18,   // Source-literal second new visible root.
            data + 2,    // Final free-space counter.
            case.definition as usize * 4096 + 16,
            152, // Original database header.
        ] {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_insert_recovery(
                    checked,
                    fixtures::intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch),
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
        // The reverse row sequence is valid data over the SAME exact BEFORE,
        // but its separately reconstructed AFTER has different slots/payloads.
        // It cannot qualify a journal for the original requested row order.
        let swapped = NativeInsertIntent::decode(
            &fixtures::wire(&case, !case.reverse),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let saved = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_insert_recovery(checked, swapped, scope(), &mut budget(), &|| false),
            Err(NativeRecoveryQualificationError::ImageMismatch),
        ));
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);
    }
}
