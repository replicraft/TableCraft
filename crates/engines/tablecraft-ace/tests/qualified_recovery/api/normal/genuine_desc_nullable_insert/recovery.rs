//! Real TCNA requalification rejects NULL-family and directional wire forgeries.
use super::{Storage, budget, fixtures, journals, scope};
use journals::Marker;
use tablecraft_ace::{NativeRecoveryQualificationError, qualify_native_insert_recovery};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_desc_nullable_insert::Case;

fn replay(case: &Case, count: usize, expected_after: &[u8], marker: Marker, initial: Vec<u8>) {
    let committed = matches!(marker, Marker::Committed);
    let outcome = if committed {
        RecoveryOutcome::Committed
    } else {
        RecoveryOutcome::RolledBack
    };
    let expected = if committed {
        expected_after
    } else {
        &case.before
    };
    let mut target = Storage::new(initial);
    let mut journal = Storage::new(journals::journal(&case.before, expected_after, marker));
    let saved = journal.bytes.get_ref().clone();
    for _ in 0..2 {
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_insert_recovery(
            checked,
            fixtures::intent(case, count),
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
        assert_eq!(target.bytes.get_ref().as_slice(), expected);
    }
    let writes = target.writes;
    if committed {
        assert_eq!(writes, 0);
    }
    target.fail_prefix = Some(0);
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_native_insert_recovery(
        checked,
        fixtures::intent(case, count),
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
#[ignore = "requires source native6 and SDK36 nullable/DESC INSERT qualification"]
fn nullable_desc_insert_requalifies_repeated_full_and_causal_guard_only_recovery() {
    let mut contexts = 0;
    for case in fixtures::cases() {
        for (count, after) in [(1, &case.single), (2, &case.batch)] {
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                replay(&case, count, after, marker, after.clone());
                contexts += 1;
            }
            let mut mixed = case.before.clone();
            let base = case.roots[1] * 4096;
            mixed[base..base + 4096].copy_from_slice(&after[base..base + 4096]);
            assert_ne!(mixed, case.before);
            assert_ne!(&mixed, after);
            for marker in [Marker::Pending, Marker::Partial] {
                replay(&case, count, after, marker, mixed.clone());
                contexts += 1;
            }
            let mut target = Storage::new(mixed.clone());
            let mut journal =
                Storage::new(journals::journal(&case.before, after, Marker::Committed));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_insert_recovery(
                checked,
                fixtures::intent(&case, count),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                qualified.recover(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            ));
            assert_eq!(target.writes, 0);
            assert_eq!(target.bytes.get_ref(), &mixed);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
    assert_eq!(contexts, 20);
}

#[test]
#[ignore = "requires source native6 and SDK36 nullable/DESC INSERT qualification"]
fn repeated_null_history_raw_mask_and_row_suffixes_are_not_caller_supplied_proof() {
    let mut refused = 0;
    for case in fixtures::cases() {
        for (count, after) in [(1, &case.single), (2, &case.batch)] {
            let guard = case.roots[1] * 4096;
            let new_null = if count == 1 { 503 } else { 512 };
            assert_eq!(after[guard + new_null], 255);
            for offset in [
                case.definition * 4096 + 79, // Repeated NULL/present Guard family cannot increment history3.
                guard + new_null,            // DESC NULL marker, not ASC NULL.
                guard + new_null + 4,        // Exact new NULL member's ROW suffix.
                case.data * 4096 + 4046, // New NULL Guard inactive payload must be the serializer's literal zero.
                case.data * 4096 + 4050, // New NULL record's presence mask.
                case.data * 4096 + 4091, // Original NULL record's inactive payload remains exact.
            ] {
                let mut forged = after.to_vec();
                if offset == guard + new_null {
                    forged[offset] = 0; // Valid ASC NULL tag cannot encode a DESC component.
                } else {
                    forged[offset] ^= 1;
                }
                let mut journal =
                    Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                assert!(matches!(
                    qualify_native_insert_recovery(
                        checked,
                        fixtures::intent(&case, count),
                        scope(),
                        &mut budget(),
                        &|| false
                    ),
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 24);
}
