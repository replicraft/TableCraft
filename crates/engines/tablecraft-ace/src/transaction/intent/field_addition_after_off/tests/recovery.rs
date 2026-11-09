use super::*;

const COMMIT_MARKER_BYTES: usize = 20;

fn qualify<'a>(
    journal: &'a mut MemoryStorage,
    wire: &[u8],
) -> Result<
    QualifiedFieldAdditionAfterOffRecovery<'a, MemoryStorage>,
    FieldAdditionAfterOffIntentError,
> {
    let checked = inspect_v2(journal, &mut budget(), &|| false).unwrap();
    qualify_native_field_addition_after_off_recovery(
        checked,
        decode(wire),
        SCOPE,
        &mut budget(),
        &|| false,
    )
}

#[test]
fn cold_original_requests_recover_pending_mixed_committed_and_partial_markers() {
    for case in [raw::SourceCase::Number, raw::SourceCase::Mixed] {
        let before = raw::source(case);
        for (column, target, after) in [
            (
                "AfterOff",
                FieldAdditionTarget::NumberLongZero,
                raw::append_number(&before, "AfterOff"),
            ),
            ("LabelOff", text(), raw::append_text(&before, "LabelOff")),
        ] {
            let wire = literal(&before, "Table1", column, target);
            for state in 0..4 {
                let mut database = MemoryStorage::new(before.clone());
                let mut journal = MemoryStorage::new(Vec::new());
                let prepared = plan(&before, column, target)
                    .prepare(&mut database, &mut journal, &mut budget(), &|| false)
                    .unwrap();
                if matches!(state, 1 | 3) {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                    if state == 3 {
                        let partial = journal.bytes.get_ref().len() - COMMIT_MARKER_BYTES;
                        journal.bytes.get_mut().truncate(partial);
                    }
                } else {
                    drop(prepared);
                    if state == 2 {
                        database.bytes.get_mut()[4 * PAGE..5 * PAGE]
                            .copy_from_slice(&after[4 * PAGE..5 * PAGE]);
                    }
                }
                let checked = qualify(&mut journal, &wire).unwrap();
                assert_eq!(
                    checked.state(),
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
                    checked
                        .recover(&mut database, &mut budget(), &|| false)
                        .unwrap(),
                    expected
                );
                assert_eq!(
                    database.bytes.get_ref(),
                    if state == 1 { &after } else { &before }
                );
                let writes = database.writes;
                assert_eq!(
                    qualify(&mut journal, &wire)
                        .unwrap()
                        .confirm_completed(&mut database, &mut budget(), &|| false)
                        .unwrap(),
                    expected
                );
                assert_eq!(database.writes, writes);
            }
        }
    }
}

#[test]
fn substituted_valid_target_name_or_generic_after_never_grants_native_authority() {
    let before = raw::source(raw::SourceCase::Number);
    let after = raw::append_text(&before, "LabelOff");
    let proper = literal(&before, "Table1", "LabelOff", text());
    for variant in 0..3 {
        let mut expected = after.clone();
        let wire = match variant {
            0 => literal(
                &before,
                "Table1",
                "LabelOff",
                FieldAdditionTarget::NumberLongZero,
            ),
            1 => literal(&before, "Table1", "OtherNew", text()),
            _ => {
                expected[10 * PAGE + 128] ^= 1;
                proper.clone()
            }
        };
        let mut database = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_expected(
                &mut database,
                &mut journal,
                &before,
                &expected,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        assert!(matches!(
            qualify(&mut journal, &wire),
            Err(FieldAdditionAfterOffIntentError::ImageMismatch)
        ));
        assert_eq!(database.bytes.get_ref(), &before);
        assert_eq!(database.writes, 0);
    }
}

#[test]
fn equal_length_profile_refuses_truncated_or_grown_target_without_writes() {
    let before = raw::source(raw::SourceCase::Number);
    let wire = literal(
        &before,
        "Table1",
        "AfterOff",
        FieldAdditionTarget::NumberLongZero,
    );
    for length in [before.len() - 2048, before.len() + 2048] {
        let mut database = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            plan(&before, "AfterOff", FieldAdditionTarget::NumberLongZero)
                .prepare(&mut database, &mut journal, &mut budget(), &|| false)
                .unwrap(),
        );
        database.bytes.get_mut().resize(length, 0);
        let unchanged = database.bytes.get_ref().clone();
        assert!(matches!(
            qualify(&mut journal, &wire)
                .unwrap()
                .recover(&mut database, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        ));
        assert_eq!(database.writes, 0);
        assert_eq!(database.bytes.get_ref(), &unchanged);
    }
}

#[test]
fn recovery_qualification_spends_one_pool_and_preserves_cancellation() {
    let before = raw::source(raw::SourceCase::Number);
    let after = raw::append_number(&before, "AfterOff");
    let wire = literal(
        &before,
        "Table1",
        "AfterOff",
        FieldAdditionTarget::NumberLongZero,
    );
    for cancelled in [false, true] {
        let mut database = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_expected(
                &mut database,
                &mut journal,
                &before,
                &after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            qualify_native_field_addition_after_off_recovery(
                checked,
                decode(&wire),
                SCOPE,
                &mut work,
                &|| cancelled
            )
            .is_err()
        );
        assert_eq!(database.writes, 0);
        assert_eq!(database.bytes.get_ref(), &before);
    }
}
