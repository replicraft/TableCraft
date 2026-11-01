//! Whole-image replay over causal prefixes of the three changed DATA pages.
use super::{Storage, budget, fixtures::cases, journals, scope, wire};
use journals::Marker;
use tablecraft_ace::qualify_native_update_recovery;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};

fn recover(case: &super::fixtures::Case, marker: Marker, initial: Vec<u8>) {
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
        let qualified = qualify_native_update_recovery(
            checked,
            wire::decode(case),
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
        wire::decode(case),
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
#[ignore = "requires all six source-qualified multirow owned images"]
fn multirow_replay_restores_partial_three_page_prefixes_and_retains_committed_mismatch() {
    let mut settled = 0;
    let mut partial = 0;
    let mut rejected_committed = 0;
    for case in cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            recover(&case, marker, case.after.clone());
            settled += 1;
        }
        // The first prefix publishes both root pointers but no terminal page;
        // the second additionally publishes only the first grown terminal.
        // Neither is a valid complete replacement or the original image.
        for last_written in &case.pages[..2] {
            let cutoff = (*last_written as usize + 1) * 4096;
            let mut mixed = case.before.clone();
            mixed[..cutoff].copy_from_slice(&case.after[..cutoff]);
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            let last = case.pages[2] as usize * 4096;
            assert_eq!(&mixed[last..last + 4096], &case.before[last..last + 4096]);
            for marker in [Marker::Pending, Marker::Partial] {
                recover(&case, marker, mixed.clone());
                partial += 1;
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
                let qualified = qualify_native_update_recovery(
                    checked,
                    wire::decode(&case),
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
            }
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_update_recovery(
                checked,
                wire::decode(&case),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                qualified.confirm_completed(&mut target, &mut budget(), &|| false),
                Err(JournalError::TargetMismatch)
            ));
            assert_eq!(target.writes, 0);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            rejected_committed += 1;
        }
    }
    assert_eq!(settled, 6);
    assert_eq!(partial, 8);
    assert_eq!(rejected_committed, 4);
}
