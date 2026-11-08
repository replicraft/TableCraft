//! Literal declaration wires and independent complete images precede journal use.
use super::*;
use crate::NativeAddColumnIntent;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, V2State, inspect_v2};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_default_zero as raw, support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(927);

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8], table: &str, name: &str) -> RecoverableDefaultZeroAddColumnPlan {
    image(before)
        .plan_recoverable_created_add_default_zero_int32_column(
            table,
            name,
            CreatedDefaultZeroColumnRequired::Absent,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn literal(before: &[u8], table: &str, column: &str, required_tag: u16) -> Vec<u8> {
    // Independent 60-byte framing and digest domain, never production constants.
    let length = 60 + table.len() + column.len();
    let mut bytes = b"TCAC0003".to_vec();
    bytes.extend((length as u32).to_le_bytes());
    bytes.extend(3_u16.to_le_bytes());
    bytes.extend(required_tag.to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    bytes.extend(digest.finalize());
    bytes.extend((table.len() as u16).to_le_bytes());
    bytes.extend((column.len() as u16).to_le_bytes());
    bytes.extend(table.as_bytes());
    bytes.extend(column.as_bytes());
    bytes
}

fn decode(bytes: &[u8]) -> NativeDefaultZeroAddColumnIntent {
    NativeDefaultZeroAddColumnIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}

#[test]
fn absent_required_request_matches_independent_wire_and_complete_growing_after() {
    for (case, table, column) in [
        (raw::Source::IdOnly, "Table1", "Score"),
        (raw::Source::ExistingZero, "Items", "Added 金額"),
    ] {
        let before = raw::source(case);
        let after = raw::append(&before, column);
        let plan = plan(&before, table, column);
        let expected = literal(&before, table, column, 0);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            expected
        );
        assert_eq!(
            decode(&expected).encode(&mut budget(), &|| false).unwrap(),
            expected
        );
        assert_eq!(
            plan.required_property(),
            CreatedDefaultZeroColumnRequired::Absent
        );
        let fresh = plan
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.before_bytes(), before);
        assert_eq!(fresh.after_bytes(), after);
        assert_eq!(after.len(), before.len() + 4096);
    }
}

#[test]
fn required_presence_is_explicit_data_and_never_cross_decoded_as_old_no_default() {
    let before = raw::source(raw::Source::IdOnly);
    for (tag, required) in [
        (0, CreatedDefaultZeroColumnRequired::Absent),
        (1, CreatedDefaultZeroColumnRequired::StoredFalse),
    ] {
        let bytes = literal(&before, "Table1", "Score", tag);
        let intent = decode(&bytes);
        assert_eq!(intent.required_property(), required);
        assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), bytes);
        assert!(NativeAddColumnIntent::decode(&bytes, &mut budget(), &|| false).is_err());
    }
    for (version, profile) in [(b'1', 1_u16), (b'2', 2), (b'3', 1), (b'4', 3)] {
        let mut bytes = literal(&before, "Table1", "Score", 0);
        bytes[7] = version;
        bytes[12..14].copy_from_slice(&profile.to_le_bytes());
        assert!(matches!(
            NativeDefaultZeroAddColumnIntent::decode(&bytes, &mut budget(), &|| false),
            Err(DefaultZeroAddColumnIntentError::InvalidRecord)
        ));
    }
}

#[test]
fn malformed_requests_refuse_before_either_borrowed_name_is_allocated() {
    let before = raw::source(raw::Source::IdOnly);
    let bytes = literal(&before, "Table1", "Score", 0);
    let mut cases = (0..bytes.len())
        .map(|end| bytes[..end].to_vec())
        .collect::<Vec<_>>();
    for offset in [0, 7, 8, 12, 14, 56, 58, 60, 66] {
        let mut invalid = bytes.clone();
        invalid[offset] = 255;
        cases.push(invalid);
    }
    let mut appended = bytes.clone();
    appended.push(0);
    cases.push(appended);
    let mut control = bytes;
    control[66] = 0;
    cases.push(control);
    for bytes in cases {
        let mut no_heap = ResourceBudget::new(0, 0, 1_000_000);
        let result = NativeDefaultZeroAddColumnIntent::decode(&bytes, &mut no_heap, &|| false);
        assert!(result.is_err());
        assert!(!matches!(
            result,
            Err(DefaultZeroAddColumnIntentError::Inspect(
                InspectError::Limit(_)
            ))
        ));
    }
}

#[test]
fn changed_original_and_required_declaration_never_requalify_the_retained_after() {
    let before = raw::source(raw::Source::IdOnly);
    let mut changed = before.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert!(matches!(
        plan(&before, "Table1", "Score").requalify_with_budgets(
            image(&changed),
            &mut budget(),
            &mut budget(),
            &|| false,
        ),
        Err(DefaultZeroAddColumnIntentError::ImageMismatch)
    ));
    let mut request_changed = plan(&before, "Table1", "Score");
    request_changed.intent.required = CreatedDefaultZeroColumnRequired::StoredFalse;
    assert!(
        request_changed
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn independent_native_and_proof_limits_and_cancellation_prevent_authority() {
    let before = raw::source(raw::Source::IdOnly);
    let exhausted = || ResourceBudget::new(0, 0, 0);
    for proof_empty in [true, false] {
        let mut native = if proof_empty { budget() } else { exhausted() };
        let mut proof = if proof_empty { exhausted() } else { budget() };
        assert!(
            plan(&before, "Table1", "Score")
                .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false,)
                .is_err()
        );
    }
    assert!(
        plan(&before, "Table1", "Score")
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| true,)
            .is_err()
    );
}

#[test]
fn original_request_recovers_pending_committed_partial_growth_and_torn_v2_marker() {
    let before = raw::source(raw::Source::IdOnly);
    let after = raw::append(&before, "Score");
    let request = literal(&before, "Table1", "Score", 0);
    for state in 0..4 {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&before, "Table1", "Score")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if matches!(state, 1 | 3) {
            prepared.commit(&mut budget(), &|| false).unwrap();
            if state == 3 {
                let partial = journal.bytes.get_ref().len() - 20;
                journal.bytes.get_mut().truncate(partial);
            }
        } else {
            drop(prepared);
            if state == 2 {
                target.bytes = Cursor::new(after[..before.len() + 2048].to_vec());
            }
        }
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_default_zero_add_column_recovery(
            checked,
            decode(&request),
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
        let outcome = qualified
            .recover(&mut target, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(
            outcome,
            if state == 1 {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(
            target.bytes.get_ref(),
            if state == 1 { &after } else { &before }
        );
    }
}

#[test]
fn forged_after_is_refused_without_target_writes_and_settled_confirmation_does_not_repair() {
    let before = raw::source(raw::Source::IdOnly);
    let after = raw::append(&before, "Score");
    let request = literal(&before, "Table1", "Score", 0);
    let mut forged = after.clone();
    *forged.last_mut().unwrap() ^= 1;
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_expected(
            &mut target,
            &mut journal,
            &before,
            &forged,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_default_zero_add_column_recovery(
            checked,
            decode(&request),
            SCOPE,
            &mut budget(),
            &|| false,
        ),
        Err(DefaultZeroAddColumnIntentError::ImageMismatch)
    ));
    assert_eq!(target.bytes.get_ref(), &before);
    assert_eq!(target.writes, 0);

    let mut journal = MemoryStorage::new(Vec::new());
    plan(&before, "Table1", "Score")
        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    let writes = target.writes;
    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_native_default_zero_add_column_recovery(
        checked,
        decode(&request),
        SCOPE,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        qualified
            .confirm_completed(&mut target, &mut budget(), &|| false)
            .unwrap(),
        RecoveryOutcome::Committed
    );
    assert_eq!(target.writes, writes);
    assert_eq!(target.bytes.get_ref(), &after);
}

#[test]
fn stored_false_number_requests_bind_all_three_independent_sdk_property_order_pairs() {
    use tablecraft_testkit::fixtures::access::canonical_created_number as number;

    for (case, table, column) in [
        (number::Source::IdOnly, "Table1", "Score"),
        (number::Source::GuiRequiredZero, "Table1", "Extra"),
        (number::Source::DefaultOnly, "Items", "Extra"),
    ] {
        let before = number::source(case);
        let after = number::append(&before, case, column);
        let plan = image(&before)
            .plan_recoverable_created_add_default_zero_int32_column(
                table,
                column,
                CreatedDefaultZeroColumnRequired::StoredFalse,
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let request = literal(&before, table, column, 1);
        assert_eq!(
            plan.required_property(),
            CreatedDefaultZeroColumnRequired::StoredFalse
        );
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            request
        );
        let fresh = plan
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.before_bytes(), before);
        assert_eq!(fresh.after_bytes(), after);
        let mut target = MemoryStorage::new(before);
        let mut journal = MemoryStorage::new(Vec::new());
        fresh
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap()
            .commit(&mut budget(), &|| false)
            .unwrap();
        let writes = target.writes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_default_zero_add_column_recovery(
            checked,
            decode(&request),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!(target.writes, writes);
        assert_eq!(target.bytes.get_ref(), &after);
    }
}
