//! Original TCND replay over full and authored causal DATA/INDEX publication states.
use super::{Storage, budget, fixtures, journals, scope};
use fixtures::Recipe;
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
            fixtures::intent(case, false),
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
        fixtures::intent(case, false),
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
#[ignore = "requires all sixteen independently qualified indexed DELETE source images"]
fn exterior_indexed_delete_recovers_every_causal_page_and_preserves_checked_journal() {
    let mut contexts = 0;
    for case in fixtures::cases()
        .into_iter()
        .filter(|case| case.recipe == Recipe::Exterior)
    {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            replay(&case, marker, case.after.clone());
            contexts += 1;
        }
        for page in [case.definition, case.data, case.roots[0], case.roots[1]] {
            let mut mixed = case.before.clone();
            mixed[page * 4096..(page + 1) * 4096]
                .copy_from_slice(&case.after[page * 4096..(page + 1) * 4096]);
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
                fixtures::intent(&case, false),
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
    }
    assert_eq!(contexts, 22);
}

#[test]
#[ignore = "requires all sixteen independently qualified indexed DELETE source images"]
fn indexed_delete_history_tails_other_tree_and_original_witnesses_are_not_journal_authority() {
    for case in fixtures::cases() {
        let data = case.data * 4096;
        let td = case.definition * 4096;
        for offset in [
            td + 67,
            td + 79,                    // Both original historical counters stay3.
            data + 4095,                // Original row0 presence/payload survives even tombstoned.
            case.roots[0] * 4096 + 507, // Original inactive INDEX tail.
            case.roots[1] * 4096 + 507,
            data + 14, // Correct original tombstone word is necessary.
            152,       // Unrelated original database header.
        ] {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_delete_recovery(
                    checked,
                    fixtures::intent(&case, false),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
        // Correct primary maintenance cannot conceal a missing Guard-tree removal.
        let mut forged = case.after.clone();
        let at = case.roots[1] * 4096;
        forged[at..at + 4096].copy_from_slice(&case.before[at..at + 4096]);
        let mut journal = Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
        let saved = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_delete_recovery(
                checked,
                fixtures::intent(&case, false),
                scope(),
                &mut budget(),
                &|| false,
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(journal.writes, 0);
        assert_eq!(journal.bytes.get_ref(), &saved);

        // An altered original witness remains data, not a historical permission.
        let mut wire = fixtures::wire(&case, false);
        wire[87] ^= 1; // First original record's Id payload, after header73+rowheader12+count2.
        let intent = NativeDeleteIntent::decode(&wire, &mut budget(), &|| false).unwrap();
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Pending,
        ));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false,),
            Err(NativeRecoveryQualificationError::Edit(EditError::Conflict))
        ));
        assert_eq!(journal.writes, 0);
    }
}
