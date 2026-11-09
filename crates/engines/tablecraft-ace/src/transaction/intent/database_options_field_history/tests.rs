//! Independent TCDO2 request framing, immutable source binding and checked V2 replay.
use super::*;
use crate::{NativeDatabaseOptionsIntent, NativeImage};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_core::limits::Resource;
use tablecraft_io::journal::{self, JournalError, RecoveryOutcome, V2State, inspect_v2};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::{Source, mixed, repeated},
        canonical_database_options,
    },
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(1044);
const BYTES: u64 = 512 * 1024 * 1024;
const STEPS: u64 = 5_000_000;
const PAGE: usize = 4096;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, 100_000, STEPS)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn sources() -> [Vec<u8>; 2] {
    let text = mixed::append_text(
        &mixed::source(mixed::SourceCase::IdOnly),
        "Label",
        255,
        true,
    );
    let number = mixed::append_number(&text, "AfterLabel");
    [
        repeated::source(Source::IdOnly),
        mixed::append_text(&number, "OtherLabel", 255, true),
    ]
}

fn plan(before: &[u8]) -> RecoverableFieldHistoryDatabaseOptionsPlan {
    image(before)
        .plan_recoverable_disable_name_auto_correct_after_field_history(&mut budget(), &|| false)
        .unwrap()
}

fn literal(before: &[u8]) -> Vec<u8> {
    // Independent 64-byte request: version2/profile2, zero flags, fixed OFF1.
    assert_eq!(before.len(), 45056);
    let mut raw = b"TCDO0002".to_vec();
    raw.extend(64_u32.to_le_bytes());
    raw.extend([2, 0, 0, 0]);
    raw.extend(45056_u64.to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update(45056_u64.to_le_bytes());
    digest.update(before);
    raw.extend(digest.finalize());
    raw.extend(1_u32.to_le_bytes());
    raw.extend(0_u32.to_le_bytes());
    assert_eq!(raw.len(), 64);
    raw
}

fn decoded(raw: &[u8]) -> NativeFieldHistoryDatabaseOptionsIntent {
    NativeFieldHistoryDatabaseOptionsIntent::decode(raw, &mut budget(), &|| false).unwrap()
}

#[test]
fn independently_framed_requests_bind_original_and_rebuild_complete_shared_images() {
    for before in sources() {
        let after = canonical_database_options::append_explicit_off_to_owned_page(&before);
        let reviewed = plan(&before);
        let raw = literal(&before);
        assert_eq!(
            reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
            raw
        );
        assert_eq!(decoded(&raw).encode(&mut budget(), &|| false).unwrap(), raw);
        let fresh = reviewed
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(fresh.before_bytes(), before);
        assert_eq!(fresh.after_bytes(), after);
        assert_eq!(fresh.property_reference() >> 8, 10);
    }
}

#[test]
fn fixed_framing_action_profile_and_versions_are_closed_without_fallback() {
    let raw = literal(&sources()[0]);
    for offset in [8, 14, 16, 56, 60] {
        let mut changed = raw.clone();
        changed[offset] ^= 0xff;
        assert!(matches!(
            NativeFieldHistoryDatabaseOptionsIntent::decode(&changed, &mut budget(), &|| false),
            Err(FieldHistoryOptionsIntentError::InvalidRecord)
        ));
    }
    for length in 0..64 {
        assert!(
            NativeFieldHistoryDatabaseOptionsIntent::decode(&raw[..length], &mut budget(), &|| {
                false
            })
            .is_err()
        );
    }
    let mut extra = raw.clone();
    extra.push(0);
    assert!(
        NativeFieldHistoryDatabaseOptionsIntent::decode(&extra, &mut budget(), &|| false).is_err()
    );
    let mut profile = raw.clone();
    profile[12] = 1;
    assert!(matches!(
        NativeFieldHistoryDatabaseOptionsIntent::decode(&profile, &mut budget(), &|| false),
        Err(FieldHistoryOptionsIntentError::UnsupportedProfile)
    ));
    let mut old = raw.clone();
    old[..8].copy_from_slice(b"TCDO0001");
    assert!(matches!(
        NativeFieldHistoryDatabaseOptionsIntent::decode(&old, &mut budget(), &|| false),
        Err(FieldHistoryOptionsIntentError::UnsupportedVersion)
    ));
    assert!(matches!(
        NativeDatabaseOptionsIntent::decode(&raw, &mut budget(), &|| false),
        Err(crate::DatabaseOptionsIntentError::UnsupportedVersion)
    ));
    let mut foreign = raw.clone();
    foreign[..8].copy_from_slice(b"TCAC0005");
    assert!(matches!(
        NativeFieldHistoryDatabaseOptionsIntent::decode(&foreign, &mut budget(), &|| false),
        Err(FieldHistoryOptionsIntentError::InvalidRecord)
    ));
}

#[test]
fn exact_encoding_quota_and_fresh_native_proof_pools_remain_independent() {
    let before = &sources()[0];
    let reviewed = plan(before);
    let mut measured = budget();
    reviewed.intent().encode(&mut measured, &|| false).unwrap();
    assert_eq!(BYTES - measured.remaining(Resource::Bytes), 64);
    assert!(
        reviewed
            .intent()
            .encode(&mut ResourceBudget::new(63, 0, STEPS), &|| false)
            .is_err()
    );
    assert!(
        NativeFieldHistoryDatabaseOptionsIntent::decode(
            &literal(before),
            &mut ResourceBudget::new(0, 0, STEPS),
            &|| false
        )
        .is_ok()
    );
    for native_empty in [false, true] {
        let mut native = if native_empty {
            ResourceBudget::new(0, 0, 0)
        } else {
            budget()
        };
        let mut proof = if native_empty {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            plan(before)
                .requalify_with_budgets(image(before), &mut native, &mut proof, &|| false)
                .is_err()
        );
    }
    assert!(
        plan(before)
            .requalify_with_budgets(image(before), &mut budget(), &mut budget(), &|| true)
            .is_err()
    );
    let mut stale = before.clone();
    stale[7 * PAGE + 3000] ^= 1;
    assert!(matches!(
        plan(before).requalify_with_budgets(image(&stale), &mut budget(), &mut budget(), &|| false),
        Err(FieldHistoryOptionsIntentError::ImageMismatch)
    ));
    let mut raw = literal(before);
    raw[24] ^= 1;
    assert!(matches!(
        decoded(&raw).verify_before(before, &mut budget(), &|| false),
        Err(FieldHistoryOptionsIntentError::BeforeMismatch)
    ));
}

#[test]
fn pending_mixed_committed_and_partial_marker_use_same_v2_and_settled_confirmation() {
    let before = &sources()[1];
    let after = canonical_database_options::append_explicit_off_to_owned_page(before);
    let raw = literal(before);
    for state in 0..4 {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(before)
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
                // Same-length per-byte BEFORE/AFTER mixture, not a missing tail.
                target.bytes.get_mut()[4 * PAGE..5 * PAGE]
                    .copy_from_slice(&after[4 * PAGE..5 * PAGE]);
            }
        }
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_field_history_database_options_recovery(
            checked,
            decoded(&raw),
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
            if state == 1 { &after } else { before }
        );
        let writes = target.writes;
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        qualify_native_field_history_database_options_recovery(
            checked,
            decoded(&raw),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .confirm_completed(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!(target.writes, writes);
    }
}

#[test]
fn equal_length_profile_refuses_torn_or_grown_targets_without_writes() {
    let before = &sources()[0];
    let raw = literal(before);
    for length in [before.len() - 2048, before.len() + 2048] {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            plan(before)
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap(),
        );
        target.bytes.get_mut().resize(length, 0);
        let unchanged = target.bytes.get_ref().clone();
        let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_field_history_database_options_recovery(
            checked,
            decoded(&raw),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert!(matches!(
            qualified.recover(&mut target, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &unchanged);
    }
}

#[test]
fn generic_journal_integrity_and_before_digest_do_not_supply_native_authority() {
    let before = &sources()[0];
    let mut after = canonical_database_options::append_explicit_off_to_owned_page(before);
    after[7 * PAGE + 3000] ^= 1;
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_expected(
            &mut target,
            &mut journal,
            before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_field_history_database_options_recovery(
            checked,
            decoded(&literal(before)),
            SCOPE,
            &mut budget(),
            &|| false
        ),
        Err(FieldHistoryOptionsIntentError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
    let mut forged_before = before.clone();
    forged_before[7 * PAGE + 3000] ^= 1;
    let raw = literal(&forged_before);
    decoded(&raw)
        .verify_before(&forged_before, &mut budget(), &|| false)
        .unwrap();
    let forged_after =
        canonical_database_options::append_explicit_off_to_owned_page(&forged_before);
    let mut target = MemoryStorage::new(forged_before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_expected(
            &mut target,
            &mut journal,
            &forged_before,
            &forged_after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(
        qualify_native_field_history_database_options_recovery(
            checked,
            decoded(&raw),
            SCOPE,
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(target.writes, 0);
}
