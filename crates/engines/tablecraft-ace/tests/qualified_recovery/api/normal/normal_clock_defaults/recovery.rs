//! Original retained civil input is replayed without any clock-provider API.
use super::super::{Storage, budget, model as journals, scope};
use super::{fixtures, sources, wire};
use journals::Marker;
use tablecraft_ace::{
    NativeClockInsertIntent, NativeRecoveryQualificationError, qualify_native_clock_insert_recovery,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

#[test]
#[ignore = "requires clock source18 before SAME CheckedV1 native replay"]
fn complete_and_causally_mixed_clock_targets_recover_repeat_and_confirm_without_resampling() {
    let sources = sources();
    let mut contexts = 0;
    for case in sources
        .cases()
        .into_iter()
        .filter(|case| case.recipe != fixtures::Recipe::Empty)
    {
        let mut mixed = case.before.to_vec();
        // Declared count is AFTER while DATA still has original rows/slots.
        mixed[7 * 4096..8 * 4096].copy_from_slice(&case.after[7 * 4096..8 * 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for partial in [false, true] {
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let initial = if partial {
                    mixed.as_slice()
                } else {
                    case.after
                };
                let expected = if committed { initial } else { case.before };
                let outcome = if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let mut target = Storage::new(initial.to_vec());
                let mut journal = Storage::new(journals::journal(case.before, case.after, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let intent =
                        NativeClockInsertIntent::decode(&wire(&case), &mut budget(), &|| false)
                            .unwrap();
                    let qualified = qualify_native_clock_insert_recovery(
                        checked,
                        intent,
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let result = qualified.recover(&mut target, &mut budget(), &|| false);
                    if partial && committed {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.writes, 0);
                    } else {
                        assert_eq!(result.unwrap(), outcome);
                    }
                    assert_eq!(target.bytes.get_ref().as_slice(), expected);
                    assert_eq!(journal.bytes.get_ref(), &saved);
                    assert_eq!(journal.writes, 0);
                }
                let writes = target.writes;
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent =
                    NativeClockInsertIntent::decode(&wire(&case), &mut budget(), &|| false)
                        .unwrap();
                let qualified = qualify_native_clock_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let result = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if partial && committed {
                    assert!(matches!(result, Err(JournalError::TargetMismatch)));
                } else {
                    assert_eq!(result.unwrap(), outcome);
                }
                assert_eq!(target.writes, writes);
                assert_eq!(target.bytes.get_ref().as_slice(), expected);
                assert_eq!(journal.bytes.get_ref(), &saved);
                assert_eq!(journal.writes, 0);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 84);
}

#[test]
#[ignore = "requires clock source18; checksum-valid AFTER or retained-input forgeries grant no effects"]
fn self_consistent_clock_journals_cannot_substitute_rows_masks_properties_or_execution_input() {
    let sources = sources();
    let mut refused = 0;
    for case in sources
        .cases()
        .into_iter()
        .filter(|case| case.recipe != fixtures::Recipe::Empty)
    {
        let expression = if case.function == fixtures::Function::Date {
            "Date()"
        } else {
            "Now()"
        };
        let payload = expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let locations = case
            .after
            .windows(payload.len())
            .enumerate()
            .filter(|(_, raw)| *raw == payload)
            .map(|(at, _)| at)
            .collect::<Vec<_>>();
        assert_eq!(locations.len(), 1);
        for at in [
            7 * 4096 + 16,
            8 * 4096 + 4072,
            8 * 4096 + 4080,
            8 * 4096 + 4083,
            8 * 4096 + 1000,
            3 * 4096 + 4095,
            152,
            locations[0],
        ] {
            let mut forged = case.after.to_vec();
            forged[at] ^= 1;
            let mut journal =
                Storage::new(journals::journal(case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent =
                NativeClockInsertIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_clock_insert_recovery(
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
            refused += 1;
        }
        let mut changed = wire(&case);
        if case.needs_clock() {
            // A different valid civil date always changes Date and Now alike.
            changed[18..20].copy_from_slice(&2004_u16.to_le_bytes());
        } else {
            // Explicit NULL never needed a reading; an extra input is refused.
            changed[16..25].copy_from_slice(&[1, 0, 0xd0, 7, 2, 29, 12, 0, 0]);
        }
        let intent = NativeClockInsertIntent::decode(&changed, &mut budget(), &|| false).unwrap();
        let mut journal = Storage::new(journals::journal(case.before, case.after, Marker::Pending));
        let saved = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(
            qualify_native_clock_insert_recovery(checked, intent, scope(), &mut budget(), &|| {
                false
            })
            .is_err()
        );
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);
        refused += 1;
    }
    assert_eq!(refused, 126);
}
