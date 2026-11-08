//! Independent fixed request and complete images through the same checked V2 session.
use super::*;
use crate::NativeImage;
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_io::journal::{self, RecoveryOutcome, V2State, inspect_v2};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_creation, canonical_creation_primary, canonical_database_options,
        canonical_native17_creation,
    },
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(1031);
const BYTES: u64 = 512 * 1024 * 1024;
const STEPS: u64 = 100_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, 1_000_000, STEPS)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn originals() -> [Vec<u8>; 2] {
    [
        canonical_native17_creation::source(
            canonical_creation::source(canonical_creation::Case::One).bytes,
        ),
        canonical_creation_primary::source(canonical_creation_primary::Case::One).bytes,
    ]
}

fn plan(before: &[u8]) -> RecoverableNativeDatabaseOptionsPlan {
    image(before)
        .plan_recoverable_disable_name_auto_correct(&mut budget(), &|| false)
        .unwrap()
}

fn literal(before: &[u8]) -> Vec<u8> {
    // Independent 64-byte fixed request: profile1, zero flags, explicit OFF action1.
    let mut raw = b"TCDO0001".to_vec();
    raw.extend_from_slice(&64_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    raw.extend_from_slice(&1_u32.to_le_bytes());
    raw.extend_from_slice(&0_u32.to_le_bytes());
    assert_eq!(raw.len(), 64);
    raw
}

fn decoded(raw: &[u8]) -> NativeDatabaseOptionsIntent {
    NativeDatabaseOptionsIntent::decode(raw, &mut budget(), &|| false).unwrap()
}

#[test]
fn fixed_literal_requests_rebuild_both_complete_independent_option_images() {
    for before in originals() {
        let after = canonical_database_options::explicit_off(&before);
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
        assert_eq!(fresh.property_page() as usize, before.len() / 4096);
    }
}

#[test]
fn closed_version_profile_action_and_lengths_refuse_without_decoder_fallback() {
    let raw = literal(&originals()[0]);
    for offset in [8, 14, 16, 56, 60] {
        let mut changed = raw.clone();
        changed[offset] ^= 0xff;
        assert!(matches!(
            NativeDatabaseOptionsIntent::decode(&changed, &mut budget(), &|| false),
            Err(DatabaseOptionsIntentError::InvalidRecord)
        ));
    }
    for length in 0..64 {
        assert!(
            NativeDatabaseOptionsIntent::decode(&raw[..length], &mut budget(), &|| false).is_err()
        );
    }
    let mut appended = raw.clone();
    appended.push(0);
    assert!(NativeDatabaseOptionsIntent::decode(&appended, &mut budget(), &|| false).is_err());
    let mut profile = raw.clone();
    profile[12] = 2;
    assert!(matches!(
        NativeDatabaseOptionsIntent::decode(&profile, &mut budget(), &|| false),
        Err(DatabaseOptionsIntentError::UnsupportedProfile)
    ));
    let mut future = raw.clone();
    future[..8].copy_from_slice(b"TCDO0002");
    assert!(matches!(
        NativeDatabaseOptionsIntent::decode(&future, &mut budget(), &|| false),
        Err(DatabaseOptionsIntentError::UnsupportedVersion)
    ));
    for magic in [b"TCAT0001", b"TCNT0001", b"TCAC0001", b"TCRN0001"] {
        let mut other = raw.clone();
        other[..8].copy_from_slice(magic);
        assert!(matches!(
            NativeDatabaseOptionsIntent::decode(&other, &mut budget(), &|| false),
            Err(DatabaseOptionsIntentError::InvalidRecord)
        ));
    }
}

#[test]
fn codec_exact_limits_and_terminal_cancellation_remain_explicit() {
    let before = &originals()[0];
    let reviewed = plan(before);
    let intent = reviewed.intent();
    let mut measured = budget();
    let polls = Cell::new(0);
    let raw = intent
        .encode(&mut measured, &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let bytes = BYTES - measured.remaining(Resource::Bytes);
    assert_eq!(bytes, 64);
    for (quota, admitted) in [(bytes, true), (bytes - 1, false)] {
        let mut limits = ResourceBudget::new(quota, 1_000_000, STEPS);
        assert_eq!(intent.encode(&mut limits, &|| false).is_ok(), admitted);
    }
    let seen = Cell::new(0);
    assert!(
        intent
            .encode(&mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() == polls.get()
            })
            .is_err()
    );
    // The decoder has no names/heap values; fixed framing needs no byte allocation.
    assert!(
        NativeDatabaseOptionsIntent::decode(&raw, &mut ResourceBudget::new(0, 0, STEPS), &|| false)
            .is_ok()
    );
    assert!(NativeDatabaseOptionsIntent::decode(&raw, &mut budget(), &|| true).is_err());
}

#[test]
fn independent_native_and_proof_pools_and_complete_source_binding_refuse_stale_work() {
    let before = &originals()[0];
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
    let mut stale = before.clone();
    stale[4 * 4096 + 512] ^= 1;
    assert!(matches!(
        plan(before).requalify_with_budgets(image(&stale), &mut budget(), &mut budget(), &|| false),
        Err(DatabaseOptionsIntentError::ImageMismatch)
    ));
    let mut raw = literal(before);
    raw[24] ^= 1;
    assert!(matches!(
        decoded(&raw).verify_before(before, &mut budget(), &|| false),
        Err(DatabaseOptionsIntentError::BeforeMismatch)
    ));
}

#[test]
fn pending_committed_torn_length_and_partial_marker_recover_through_same_v2_session() {
    for before in originals() {
        let after = canonical_database_options::explicit_off(&before);
        let raw = literal(&before);
        for state in 0..4 {
            let mut target = MemoryStorage::new(before.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            let prepared = plan(&before)
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            if matches!(state, 1 | 3) {
                prepared.commit(&mut budget(), &|| false).unwrap();
                if state == 3 {
                    // Independent V2 wire golden: keep half the 40-byte commit marker.
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
            let qualified = qualify_native_database_options_recovery(
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
            let expected = if state == 1 { &after } else { &before };
            assert_eq!(target.bytes.get_ref(), expected);
            let writes = target.writes;
            let checked = inspect_v2(&mut journal, &mut budget(), &|| false).unwrap();
            qualify_native_database_options_recovery(
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
}

#[test]
fn arbitrary_journal_after_and_stale_prepare_never_gain_option_authority() {
    let before = &originals()[1];
    let raw = literal(before);
    let mut after = canonical_database_options::explicit_off(before);
    let last = after.len() - 1;
    after[last] ^= 1;
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
        qualify_native_database_options_recovery(
            checked,
            decoded(&raw),
            SCOPE,
            &mut budget(),
            &|| false
        ),
        Err(DatabaseOptionsIntentError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
    let mut stale = before.clone();
    stale[4 * 4096 + 512] ^= 1;
    let mut target = MemoryStorage::new(stale);
    let mut journal = MemoryStorage::new(Vec::new());
    assert!(
        plan(before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
}
