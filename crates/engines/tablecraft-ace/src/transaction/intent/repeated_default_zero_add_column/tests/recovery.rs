use super::*;

#[test]
fn equal_length_v2_recovers_pending_committed_partial_and_torn_states() {
    let before = raw::source(Source::IdOnly);
    let after = raw::append(&before, "Score2");
    let wire = literal(&before, "Table1", "Score2");
    for state in 0..4 {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&before, "Table1", "Score2")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if matches!(state, 1 | 3) {
            prepared.commit(&mut budget(), &|| false).unwrap();
            if state == 3 {
                let incomplete_marker = journal.bytes.get_ref().len() - 20;
                journal.bytes.get_mut().truncate(incomplete_marker);
            }
        } else {
            drop(prepared);
            if state == 2 {
                target.bytes.get_mut()[4 * 4096..5 * 4096]
                    .copy_from_slice(&after[4 * 4096..5 * 4096]);
            }
        }
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_repeated_default_zero_add_column_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified.state(),
            if state == 1 {
                V2State::Committed
            } else {
                V2State::Pending
            }
        );
        let expected = if state == 1 {
            RecoveryOutcome::Committed
        } else {
            RecoveryOutcome::RolledBack
        };
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
            expected
        );
        assert_eq!(
            target.bytes.get_ref(),
            if state == 1 { &after } else { &before }
        );
        let writes = target.writes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let settled = qualify_native_repeated_default_zero_add_column_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .confirm_completed(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(settled, expected);
        assert_eq!(target.writes, writes, "settled confirmation cannot repair");
    }
}

#[test]
fn valid_generic_after_with_changed_history_gap_is_not_native_authority() {
    let before = raw::source(Source::IdOnly);
    let mut after = raw::append(&before, "Score2");
    assert_eq!(after[10 * 4096 + 128], 0);
    after[10 * 4096 + 128] = 1;
    let wire = literal(&before, "Table1", "Score2");
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_expected(
            &mut target,
            &mut journal,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_repeated_default_zero_add_column_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false,
        ),
        Err(RepeatedDefaultZeroAddColumnIntentError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}

#[test]
fn target_drift_and_recovery_cancellation_or_quota_never_write() {
    let before = raw::source(Source::IdOnly);
    let wire = literal(&before, "Table1", "Score2");
    let mut drift = before.clone();
    *drift.last_mut().unwrap() ^= 1;
    let mut target = MemoryStorage::new(drift.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    assert!(
        plan(&before, "Table1", "Score2")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false,)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
    assert_eq!(target.bytes.get_ref(), &drift);
    for cancelled in [false, true] {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            plan(&before, "Table1", "Score2")
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap(),
        );
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            qualify_native_repeated_default_zero_add_column_recovery(
                checked,
                decode(&wire),
                SCOPE,
                &mut work,
                &|| cancelled,
            )
            .is_err()
        );
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}
