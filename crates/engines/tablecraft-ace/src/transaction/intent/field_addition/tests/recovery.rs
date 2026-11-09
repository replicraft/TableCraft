use super::*;
use tablecraft_testkit::fixtures::access::canonical_created_number::{
    Source as NumberSource, repeated as number_history,
};

const PAGE_BYTES: usize = 4096;
const CATALOG_PAGE: usize = 4;
const PROPERTY_PAGE: usize = 10;
const JOURNAL_COMMIT_BYTES: usize = 20;

fn recovery_case(before: &[u8], after: &[u8], column: &str, target: FieldAdditionTarget) {
    let wire = literal(before, "Table1", column, target);
    for state in 0..4 {
        let mut database = MemoryStorage::new(before.to_vec());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(before, column, target)
            .prepare(&mut database, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if matches!(state, 1 | 3) {
            prepared.commit(&mut budget(), &|| false).unwrap();
            if state == 3 {
                let incomplete = journal.bytes.get_ref().len() - JOURNAL_COMMIT_BYTES;
                journal.bytes.get_mut().truncate(incomplete);
            }
        } else {
            drop(prepared);
            if state == 2 {
                database.bytes.get_mut()
                    [CATALOG_PAGE * PAGE_BYTES..(CATALOG_PAGE + 1) * PAGE_BYTES]
                    .copy_from_slice(
                        &after[CATALOG_PAGE * PAGE_BYTES..(CATALOG_PAGE + 1) * PAGE_BYTES],
                    );
                database
                    .bytes
                    .get_mut()
                    .extend_from_slice(&after[before.len()..]);
            }
        }
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_field_addition_recovery(
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
                .recover(&mut database, &mut budget(), &|| false)
                .unwrap(),
            expected
        );
        assert_eq!(
            database.bytes.get_ref(),
            if state == 1 { after } else { before }
        );
        let writes = database.writes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let settled = qualify_native_field_addition_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .confirm_completed(&mut database, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(settled, expected);
        assert_eq!(
            database.writes, writes,
            "settled confirmation cannot repair"
        );
    }
}

#[test]
fn cold_original_request_recovers_growing_and_fitted_pending_committed_partial_torn_images() {
    let before = raw::source(raw::SourceCase::IdOnly);
    recovery_case(
        &before,
        &raw::append_text(&before, "Label", 255, true),
        "Label",
        text(255, true),
    );
    let before = raw::append_text(&before, "Label", 255, true);
    recovery_case(
        &before,
        &raw::append_number(&before, "AfterLabel"),
        "AfterLabel",
        FieldAdditionTarget::NumberLongZero,
    );
}

#[test]
fn valid_substituted_width_compression_or_target_never_qualifies_an_unrelated_after() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let after = raw::append_text(&before, "Label", 255, true);
    for target in [
        text(80, true),
        text(255, false),
        FieldAdditionTarget::NumberLongZero,
    ] {
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
        assert!(matches!(
            qualify_native_field_addition_recovery(
                checked,
                decode(&literal(&before, "Table1", "Label", target)),
                SCOPE,
                &mut budget(),
                &|| false
            ),
            Err(FieldAdditionIntentError::ImageMismatch)
        ));
        assert_eq!(database.bytes.get_ref(), &before);
        assert_eq!(database.writes, 0);
    }
}

#[test]
fn generic_journal_after_tampering_never_grants_native_authority() {
    let before = raw::append_text(&raw::source(raw::SourceCase::IdOnly), "Label", 255, true);
    let mut after = raw::append_number(&before, "AfterLabel");
    let gap = PROPERTY_PAGE * PAGE_BYTES + 128;
    assert_eq!(after[gap], 0);
    after[gap] = 1;
    let wire = literal(
        &before,
        "Table1",
        "AfterLabel",
        FieldAdditionTarget::NumberLongZero,
    );
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
    assert!(matches!(
        qualify_native_field_addition_recovery(
            checked,
            decode(&wire),
            SCOPE,
            &mut budget(),
            &|| false
        ),
        Err(FieldAdditionIntentError::ImageMismatch)
    ));
    assert_eq!(database.bytes.get_ref(), &before);
    assert_eq!(database.writes, 0);
}

#[test]
fn recovery_uses_one_cumulative_allowance_and_preserves_cancellation() {
    let before = raw::source(raw::SourceCase::IdOnly);
    let after = raw::append_text(&before, "Label", 80, true);
    let wire = literal(&before, "Table1", "Label", text(80, true));
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
            qualify_native_field_addition_recovery(
                checked,
                decode(&wire),
                SCOPE,
                &mut work,
                &|| cancelled
            )
            .is_err()
        );
        assert_eq!(database.bytes.get_ref(), &before);
        assert_eq!(database.writes, 0);
    }
}

#[test]
fn typed_number_intent_reconstructs_existing_independent_eof_rollover_images() {
    let mut before = number_history::source(NumberSource::IdOnly);
    for ordinal in 2..=13 {
        before = number_history::append(&before, &format!("N{ordinal}"));
    }
    let after = number_history::append(&before, "N14");
    assert_eq!(before.len(), 11 * PAGE_BYTES);
    assert_eq!(after.len(), 12 * PAGE_BYTES);
    recovery_case(&before, &after, "N14", FieldAdditionTarget::NumberLongZero);
}
