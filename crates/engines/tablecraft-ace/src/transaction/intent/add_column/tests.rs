//! Independent request bytes and complete-image journal/reconstruction controls.
use super::*;
use crate::NativeImage;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_io::journal::{RecoveryOutcome, V1State, inspect_v1};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_add_column, support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(914);

fn budget() -> ResourceBudget {
    ResourceBudget::new(128 * 1024 * 1024, 1_000_000, 100_000_000)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8], name: &str) -> RecoverableAddColumnPlan {
    image(before)
        .plan_recoverable_add_int32_column("Items", name, &mut budget(), &|| false)
        .unwrap()
}

fn literal(before: &[u8], column: &str) -> Vec<u8> {
    // Independent 60-byte request header, without production framing constants.
    let mut bytes = b"TCAC0001".to_vec();
    bytes.extend_from_slice(&(65_u32 + column.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    bytes.extend_from_slice(&digest.finalize());
    bytes.extend_from_slice(&5_u16.to_le_bytes());
    bytes.extend_from_slice(&(column.len() as u16).to_le_bytes());
    bytes.extend_from_slice(b"Items");
    bytes.extend_from_slice(column.as_bytes());
    bytes
}

fn decode(bytes: &[u8]) -> NativeAddColumnIntent {
    NativeAddColumnIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}

#[test]
fn literal_requests_reconstruct_complete_independent_candidates() {
    for name in ["Extra", "Added field", "金額"] {
        let pair = canonical_add_column::source(name);
        let reviewed = plan(&pair.before, name);
        let raw = literal(&pair.before, name);
        assert_eq!(
            reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
            raw
        );
        assert_eq!(decode(&raw).table(), "Items");
        assert_eq!(decode(&raw).column(), name);
        let fresh = reviewed
            .requalify_with_budgets(image(&pair.before), &mut budget(), &mut budget(), &|| false)
            .unwrap();
        assert!(fresh.before_bytes() == pair.before);
        assert!(fresh.after_bytes() == pair.after);
        assert_eq!(fresh.table(), "Items");
        assert_eq!(fresh.column(), name);
    }
}

#[test]
fn malformed_closed_framing_refuses_before_either_name_allocation() {
    let pair = canonical_add_column::source("Extra");
    let raw = literal(&pair.before, "Extra");
    let mut cases = (0..raw.len())
        .map(|end| raw[..end].to_vec())
        .collect::<Vec<_>>();
    for offset in [0, 7, 8, 12, 14, 56, 58, 60, 65] {
        let mut altered = raw.clone();
        altered[offset] = if offset >= 60 { 0 } else { 255 };
        cases.push(altered);
    }
    let mut appended = raw.clone();
    appended.push(0);
    cases.push(appended);
    for bytes in cases {
        let mut work = ResourceBudget::new(0, 0, 1_000_000);
        let result = NativeAddColumnIntent::decode(&bytes, &mut work, &|| false);
        assert!(result.is_err());
        assert!(!matches!(
            result,
            Err(AddColumnIntentError::Inspect(InspectError::Limit(_)))
        ));
    }
}

#[test]
fn fresh_source_and_complete_replacement_cannot_be_substituted() {
    let pair = canonical_add_column::source("Extra");
    let mut stale = pair.before.clone();
    *stale.last_mut().unwrap() ^= 1;
    assert!(matches!(
        plan(&pair.before, "Extra").requalify_with_budgets(
            image(&stale),
            &mut budget(),
            &mut budget(),
            &|| false,
        ),
        Err(AddColumnIntentError::ImageMismatch)
    ));
    let mut target = MemoryStorage::new(pair.before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        plan(&pair.before, "Extra")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap(),
    );
    let mut other_request = literal(&pair.before, "Other");
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_add_column_recovery(
            checked,
            decode(&other_request),
            SCOPE,
            &mut budget(),
            &|| false
        ),
        Err(AddColumnIntentError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
    other_request[24] ^= 1;
    assert!(matches!(
        decode(&other_request).verify_before(&pair.before, &mut budget(), &|| false),
        Err(AddColumnIntentError::BeforeMismatch)
    ));
}

#[test]
fn pending_committed_and_torn_sessions_recover_only_the_reconstructed_images() {
    for state in 0..4 {
        let pair = canonical_add_column::source("Extra");
        let mut target = MemoryStorage::new(pair.before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&pair.before, "Extra")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if state == 1 || state == 3 {
            prepared.commit(&mut budget(), &|| false).unwrap();
            if state == 3 {
                let end = journal.bytes.get_ref().len() - 20;
                journal.bytes.get_mut().truncate(end);
            }
        } else {
            drop(prepared);
            if state == 2 {
                target.bytes.get_mut().copy_from_slice(&pair.after);
            }
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_add_column_recovery(
            checked,
            decode(&literal(&pair.before, "Extra")),
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
        assert!(
            target.bytes.get_ref()
                == if state == 1 {
                    &pair.after
                } else {
                    &pair.before
                }
        );
    }
}

#[test]
fn settled_confirmation_never_repairs_and_proof_pool_is_independent() {
    let pair = canonical_add_column::source("Extra");
    assert!(
        plan(&pair.before, "Extra")
            .requalify_with_budgets(
                image(&pair.before),
                &mut budget(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false,
            )
            .is_err()
    );
    assert!(
        NativeAddColumnIntent::decode(&literal(&pair.before, "Extra"), &mut budget(), &|| true)
            .is_err()
    );
    let mut target = MemoryStorage::new(pair.before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    plan(&pair.before, "Extra")
        .prepare(&mut target, &mut journal, &mut budget(), &|| false)
        .unwrap()
        .commit(&mut budget(), &|| false)
        .unwrap();
    let writes = target.writes;
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    let qualified = qualify_native_add_column_recovery(
        checked,
        decode(&literal(&pair.before, "Extra")),
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
}
