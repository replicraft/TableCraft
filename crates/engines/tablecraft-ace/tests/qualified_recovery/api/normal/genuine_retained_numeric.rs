//! Public TCNI4 plans and recovery over independent retained numeric indexes.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeFixedIntentGroup, NativeImage,
    NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    RecoverableEditError, qualify_native_update_recovery,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::genuine_retained_numeric::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn groups(expected: i32) -> Vec<NativeFixedIntentGroup> {
    vec![
        NativeFixedIntentGroup::new(
            "Marker",
            vec![FixedScalarUpdate::new(
                0,
                Value::Int32(expected),
                Value::Int32(42),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

fn wire(case: &Case) -> Vec<u8> {
    let total = 72 + case.table.len() + 12 + 6 + 26;
    let mut encoded = Vec::from(*b"TCNI0004");
    encoded.extend_from_slice(&(total as u32).to_le_bytes());
    encoded.extend_from_slice(&[4, 0, 0, 0]);
    encoded.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    encoded.extend_from_slice(&digest.finalize());
    for number in [case.table.len() as u32, 1, 1, 0] {
        encoded.extend_from_slice(&number.to_le_bytes());
    }
    encoded.extend_from_slice(case.table.as_bytes());
    for number in [6_u32, 1, 0] {
        encoded.extend_from_slice(&number.to_le_bytes());
    }
    encoded.extend_from_slice(b"Marker");
    encoded.extend_from_slice(&0_u64.to_le_bytes());
    for number in [17_u64, 42] {
        encoded.push(2);
        encoded.extend_from_slice(&number.to_le_bytes());
    }
    assert_eq!(encoded.len(), total);
    encoded
}

fn intent(case: &Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires all pinned retained numeric originals and expected images"]
fn retained_numeric_eight_commits_preserve_complete_index_and_opaque_complement() {
    let mut count = 0;
    for case in cases() {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates(case.table, groups(17), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(
            plan.intent().profile(),
            NativeUpdateProfile::Ace16OrdinaryFixed
        );
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        let marker = (case.data as usize + 1) * 4096 - 5;
        assert_eq!(&case.after[..marker], &case.before[..marker]);
        assert_eq!(&case.after[marker + 4..], &case.before[marker + 4..]);
        assert_eq!(&case.after[marker..marker + 4], &[42, 0, 0, 0]);
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 1);
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed)
        );
        count += 1;
    }
    assert_eq!(count, 8);
}

#[test]
#[ignore = "requires all pinned retained numeric originals and expected images"]
fn retained_numeric_original_intent_recovery_and_write_denied_confirmation() {
    let mut count = 0;
    for case in cases() {
        // A 17→42 Int32 replacement has only one differing byte: an interrupted
        // physical prefix is necessarily one complete observed image here.
        assert_eq!(
            case.before
                .iter()
                .zip(&case.after)
                .filter(|(a, b)| a != b)
                .count(),
            1
        );
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let outcome = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let expected = if committed { &case.after } else { &case.before };
            let mut target = Storage::new(case.after.clone());
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let journal_before = journal.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_update_recovery(
                    checked,
                    intent(&case),
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
            let qualified = qualify_native_update_recovery(
                checked,
                intent(&case),
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
            assert_eq!(journal.bytes.get_ref(), &journal_before);
            count += 1;
        }
    }
    assert_eq!(count, 24);
}

#[test]
#[ignore = "requires all pinned retained numeric originals and expected images"]
fn retained_numeric_stale_requests_and_protected_after_forgeries_refuse() {
    let mut stale = 0;
    let mut forgeries = 0;
    for case in cases() {
        let result = image(&case.before).plan_recoverable_ordinary_fixed_updates(
            case.table,
            groups(18),
            &mut budget(),
            &|| false,
        );
        assert!(matches!(
            result,
            Err(RecoverableEditError::Edit(EditError::Conflict))
        ));
        stale += 1;
        for at in [2 * 4096 + 16, case.data as usize * 4096 + 4] {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &after, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(
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
            forgeries += 1;
        }
    }
    assert_eq!(stale, 8);
    assert_eq!(forgeries, 16);
}
