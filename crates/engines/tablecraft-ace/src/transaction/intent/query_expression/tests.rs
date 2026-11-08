//! Independent TCQE framing and original-intent recovery observations.
use super::*;
use crate::transaction::qualified_recovery::NativeRecoveryQualificationError;
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::Resource;
use tablecraft_io::journal::{self, RecoveryOutcome, V1State, inspect_v1};
use tablecraft_testkit::{
    fixtures::access::saved_queries::fitting_select::storage_pair,
    support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(127);
const BYTES: u64 = 512 * 1024 * 1024;
const ROWS: u64 = 1_000_000;
const STEPS: u64 = 500_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, ROWS, STEPS)
}

fn image(before: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8]) -> RecoverableNativeQueryExpressionPlan {
    image(before)
        .plan_recoverable_query_literal_expression("Saved", "42", "43", &mut budget(), &|| false)
        .unwrap()
}

fn decoded(raw: &[u8]) -> NativeQueryExpressionIntent {
    NativeQueryExpressionIntent::decode(raw, &mut budget(), &|| false).unwrap()
}

fn literal(before: &[u8]) -> Vec<u8> {
    // Independent 80-byte header plus Saved/42/43 original request. Production
    // offsets, encoder and captured intent are deliberately not consulted.
    let mut raw = b"TCQE0001".to_vec();
    raw.extend_from_slice(&89_u32.to_le_bytes());
    raw.extend_from_slice(&1_u16.to_le_bytes());
    raw.extend_from_slice(&0_u16.to_le_bytes());
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    raw.extend_from_slice(&(-2147483525_i32).to_le_bytes());
    raw.extend_from_slice(&0x0f00_0001_i32.to_le_bytes());
    raw.extend_from_slice(&4_u32.to_le_bytes());
    raw.extend_from_slice(&[8, 0, 0, 0]);
    raw.extend_from_slice(&5_u16.to_le_bytes());
    raw.extend_from_slice(&2_u16.to_le_bytes());
    raw.extend_from_slice(&2_u16.to_le_bytes());
    raw.extend_from_slice(&0_u16.to_le_bytes());
    raw.extend_from_slice(b"Saved4243");
    raw
}

#[test]
fn independent_wire_and_complete_after_survive_fresh_requalification() {
    let [before, after] = storage_pair();
    let reviewed = plan(&before);
    let raw = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(raw, literal(&before));
    assert_eq!(decoded(&raw).encode(&mut budget(), &|| false).unwrap(), raw);
    let fresh = reviewed
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fresh.before_bytes(), before);
    assert_eq!(fresh.after_bytes(), after);
    assert_eq!(fresh.query(), "Saved");
    assert_eq!(fresh.native_id(), -2147483525);
    assert!(fresh.changed());
}

#[test]
fn malformed_closed_wire_refuses_before_allocation_without_action_fallback() {
    let [before, _] = storage_pair();
    let raw = literal(&before);
    let mut cases = Vec::new();
    for offset in [8, 14, 69, 70, 71, 72, 74, 76, 78, 80, 85, 87] {
        let mut changed = raw.clone();
        changed[offset] ^= 0xff;
        cases.push(changed);
    }
    for end in 0..raw.len() {
        cases.push(raw[..end].to_vec());
    }
    let mut appended = raw.clone();
    appended.push(0);
    cases.push(appended);
    for altered in cases {
        let mut limits = ResourceBudget::new(0, 0, STEPS);
        let error = NativeQueryExpressionIntent::decode(&altered, &mut limits, &|| false)
            .err()
            .expect("malformed wire must refuse");
        assert!(!matches!(
            error,
            QueryExpressionIntentError::Inspect(InspectError::Limit(_))
        ));
        assert_eq!(limits.remaining(Resource::Bytes), 0);
    }
    for other_action in [b"TCNI0001", b"TCNA0001", b"TCND0001", b"TCRN0001"] {
        let mut changed = raw.clone();
        changed[..8].copy_from_slice(other_action);
        assert!(matches!(
            NativeQueryExpressionIntent::decode(&changed, &mut budget(), &|| false),
            Err(QueryExpressionIntentError::InvalidRecord)
        ));
    }
    let mut future = raw.clone();
    future[7] = b'2';
    assert!(matches!(
        NativeQueryExpressionIntent::decode(&future, &mut budget(), &|| false),
        Err(QueryExpressionIntentError::UnsupportedVersion)
    ));
    let mut future_profile = raw;
    future_profile[12] = 2;
    assert!(matches!(
        NativeQueryExpressionIntent::decode(&future_profile, &mut budget(), &|| false),
        Err(QueryExpressionIntentError::UnsupportedProfile)
    ));
}

#[test]
fn stale_original_and_tampered_identity_or_request_cannot_requalify() {
    let [before, _] = storage_pair();
    let mut stale = before.clone();
    let last = stale.len() - 1;
    stale[last] ^= 1;
    assert!(matches!(
        plan(&before)
            .requalify_with_budgets(image(&stale), &mut budget(), &mut budget(), &|| false,),
        Err(QueryExpressionRecoveryError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    let mut altered = plan(&before);
    altered.intent.identity.native_id += 1;
    assert!(matches!(
        altered.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(QueryExpressionRecoveryError::Intent(
            QueryExpressionIntentError::IdentityMismatch
        ))
    ));
    let mut altered = plan(&before);
    altered.intent.replacement = "44".to_owned();
    assert!(matches!(
        altered.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(QueryExpressionRecoveryError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
}

#[test]
fn native_and_proof_pools_remain_independent_and_terminal_cancellation_refuses() {
    let [before, _] = storage_pair();
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
            plan(&before)
                .requalify_with_budgets(image(&before), &mut native, &mut proof, &|| false)
                .is_err()
        );
    }
    let polls = Cell::new(0);
    plan(&before)
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let seen = Cell::new(0);
    assert!(
        plan(&before)
            .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| {
                seen.set(seen.get() + 1);
                seen.get() == polls.get()
            })
            .is_err()
    );
}

#[test]
fn pending_committed_and_torn_marker_sessions_recover_and_confirm_without_replay() {
    let [before, after] = storage_pair();
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
                // Independent V1 marker observation: only half of its 40 bytes remain.
                let partial = journal.bytes.get_ref().len() - 20;
                journal.bytes.get_mut().truncate(partial);
            }
        } else {
            drop(prepared);
            if state == 2 {
                target.bytes.get_mut().copy_from_slice(&after);
            }
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_query_expression_recovery(
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
                V1State::Committed
            } else {
                V1State::Pending
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
        let writes = target.writes;
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        qualify_native_query_expression_recovery(
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
fn patched_after_and_stale_target_never_gain_query_authority() {
    let [before, mut after] = storage_pair();
    let raw = literal(&before);
    let last = after.len() - 1;
    after[last] ^= 1;
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut journal,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = target.writes;
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_query_expression_recovery(
            checked,
            decoded(&raw),
            SCOPE,
            &mut budget(),
            &|| false,
        ),
        Err(QueryExpressionRecoveryError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    assert_eq!(target.writes, writes);
    let mut stale = before.clone();
    stale[last] ^= 1;
    let mut target = MemoryStorage::new(stale);
    let mut journal = MemoryStorage::new(Vec::new());
    assert!(
        plan(&before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
}

#[test]
fn exact_decode_quotas_and_final_cancellation_are_enforced() {
    let [before, _] = storage_pair();
    let raw = literal(&before);
    let mut measured = budget();
    let polls = Cell::new(0);
    NativeQueryExpressionIntent::decode(&raw, &mut measured, &|| {
        polls.set(polls.get() + 1);
        false
    })
    .unwrap();
    for (resource, available) in [(Resource::Bytes, BYTES), (Resource::Steps, STEPS)] {
        let needed = available - measured.remaining(resource);
        assert!(needed > 0);
        for (quota, admitted) in [(needed, true), (needed - 1, false)] {
            let mut limits = match resource {
                Resource::Bytes => ResourceBudget::new(quota, ROWS, STEPS),
                Resource::Steps => ResourceBudget::new(BYTES, ROWS, quota),
                Resource::Rows => unreachable!(),
            };
            assert_eq!(
                NativeQueryExpressionIntent::decode(&raw, &mut limits, &|| false).is_ok(),
                admitted
            );
        }
    }
    let seen = Cell::new(0);
    assert!(
        NativeQueryExpressionIntent::decode(&raw, &mut budget(), &|| {
            seen.set(seen.get() + 1);
            seen.get() == polls.get()
        })
        .is_err()
    );
}
