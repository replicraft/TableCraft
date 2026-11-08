//! Independent request bytes and complete reconstruction over a fixed journal.
use super::*;
use sha2::{Digest, Sha256};
use std::cell::Cell;
use tablecraft_io::journal::{self, RecoveryOutcome, V1State, inspect_v1};
use tablecraft_testkit::{
    fixtures::access::query_expression_edits as raw, support::io::journal::MemoryStorage,
};

const SCOPE: CatalogScope = CatalogScope::new(128);
const BYTES: u64 = 512 * 1024 * 1024;
const ROWS: u64 = 1_000_000;
const STEPS: u64 = 500_000_000;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, ROWS, STEPS)
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), SCOPE, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8]) -> RecoverableNativeQueryExpressionEditsPlan {
    let edits = [
        QueryFragmentExpressionEdit {
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 2],
            expected: "99",
            replacement: "7000",
        },
        QueryFragmentExpressionEdit {
            attribute: QueryAttribute::Column,
            order: [0, 0, 0, 1],
            expected: "42",
            replacement: "1000",
        },
    ];
    image(before)
        .plan_recoverable_query_expression_edits("Saved", &edits, &mut budget(), &|| false)
        .unwrap()
}

fn decoded(bytes: &[u8]) -> NativeQueryExpressionEditsIntent {
    NativeQueryExpressionEditsIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}

fn expected_after(before: &[u8]) -> Vec<u8> {
    raw::replace_at(&raw::replace_at(before, 4, "7000"), 3, "1000")
}

fn literal(before: &[u8]) -> Vec<u8> {
    // Independent 80-byte header; Saved then two 12-byte entry headers and text.
    // Expected bytes do not import production offsets, tags, limits or encoders.
    let mut wire = b"TCQE0002".to_vec();
    wire.extend_from_slice(&121_u32.to_le_bytes());
    wire.extend_from_slice(&1_u16.to_le_bytes());
    wire.extend_from_slice(&0_u16.to_le_bytes());
    wire.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    wire.extend_from_slice(&hash.finalize());
    wire.extend_from_slice(&(-2147483525_i32).to_le_bytes());
    wire.extend_from_slice(&0x0f00_0001_i32.to_le_bytes());
    wire.extend_from_slice(&4_u32.to_le_bytes());
    wire.extend_from_slice(&[8, 0, 0, 0]);
    wire.extend_from_slice(&5_u16.to_le_bytes());
    wire.extend_from_slice(&2_u16.to_le_bytes());
    wire.extend_from_slice(&36_u32.to_le_bytes());
    wire.extend_from_slice(b"Saved");
    wire.extend_from_slice(&[6, 0, 2, 0, 4, 0, 0, 0, 0, 0, 0, 2]);
    wire.extend_from_slice(b"997000");
    wire.extend_from_slice(&[6, 0, 2, 0, 4, 0, 0, 0, 0, 0, 0, 1]);
    wire.extend_from_slice(b"421000");
    assert_eq!(wire.len(), 121);
    wire
}

#[test]
fn independent_wire_and_ordered_whole_candidate_survive_fresh_requalification() {
    let before = raw::two_projections();
    let reviewed = plan(&before);
    let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
    assert_eq!(wire, literal(&before));
    assert_eq!(
        decoded(&wire).encode(&mut budget(), &|| false).unwrap(),
        wire
    );
    assert_eq!(decoded(&wire).edits()[0].as_request().order, [0, 0, 0, 2]);
    assert_eq!(decoded(&wire).edits()[1].as_request().order, [0, 0, 0, 1]);

    let fresh = reviewed
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fresh.before_bytes(), before);
    assert_eq!(fresh.after_bytes(), expected_after(&before));
    assert_eq!(fresh.query(), "Saved");
    assert_eq!(fresh.native_id(), -2147483525);
    assert!(fresh.changed());
}

#[test]
fn malformed_closed_entries_refuse_before_owned_allocation() {
    let before = raw::two_projections();
    let wire = literal(&before);
    let mut malformed = Vec::new();
    for offset in [8, 14, 69, 72, 74, 76, 85, 86, 87, 91, 97, 104, 109] {
        let mut altered = wire.clone();
        altered[offset] ^= 0xff;
        malformed.push(altered);
    }
    for end in 0..wire.len() {
        malformed.push(wire[..end].to_vec());
    }
    let mut appended = wire.clone();
    appended.push(0);
    malformed.push(appended);
    let mut duplicate_slot = wire;
    duplicate_slot[111..115].copy_from_slice(&[0, 0, 0, 2]);
    malformed.push(duplicate_slot);

    for altered in malformed {
        let mut limits = ResourceBudget::new(0, 0, STEPS);
        let failure = NativeQueryExpressionEditsIntent::decode(&altered, &mut limits, &|| false)
            .err()
            .expect("invalid framing must refuse without owned allocations");
        assert!(!matches!(
            failure,
            QueryExpressionEditsIntentError::Inspect(InspectError::Limit(_))
        ));
        assert_eq!(limits.remaining(Resource::Bytes), 0);
    }
}

#[test]
fn legacy_and_future_versions_never_fall_back_to_another_request_language() {
    let before = raw::two_projections();
    let wire = literal(&before);
    assert!(matches!(
        crate::NativeQueryExpressionIntent::decode(&wire, &mut budget(), &|| false),
        Err(crate::QueryExpressionIntentError::UnsupportedVersion)
    ));
    let legacy = image(&raw::source())
        .plan_recoverable_query_literal_expression("Saved", "42", "43", &mut budget(), &|| false)
        .unwrap()
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    assert!(matches!(
        NativeQueryExpressionEditsIntent::decode(&legacy, &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::UnsupportedVersion)
    ));
    for action in [b"TCNI0001", b"TCNA0001", b"TCND0001", b"TCRN0001"] {
        let mut other = wire.clone();
        other[..8].copy_from_slice(action);
        assert!(matches!(
            NativeQueryExpressionEditsIntent::decode(&other, &mut budget(), &|| false),
            Err(QueryExpressionEditsIntentError::InvalidRecord)
        ));
    }
    let mut future = wire.clone();
    future[7] = b'3';
    assert!(matches!(
        NativeQueryExpressionEditsIntent::decode(&future, &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::UnsupportedVersion)
    ));
    let mut profile = wire;
    profile[12] = 2;
    assert!(matches!(
        NativeQueryExpressionEditsIntent::decode(&profile, &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::UnsupportedProfile)
    ));
}

#[test]
fn source_identity_and_ordered_request_tampering_cannot_requalify() {
    let before = raw::two_projections();
    let mut stale = before.clone();
    let last = stale.len() - 1;
    stale[last] ^= 1;
    assert!(matches!(
        plan(&before)
            .requalify_with_budgets(image(&stale), &mut budget(), &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));

    let mut altered = plan(&before);
    altered.intent.identity.native_id += 1;
    assert!(matches!(
        altered.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::IdentityMismatch)
    ));
    let mut altered = plan(&before);
    altered.intent.edits[0].replacement = "8000".to_owned();
    assert!(matches!(
        altered.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    let mut altered = plan(&before);
    altered.intent.edits.swap(0, 1);
    assert!(matches!(
        altered.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    let mut altered = plan(&before);
    altered.intent.before_binding[0] ^= 1;
    assert!(matches!(
        altered.requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false),
        Err(QueryExpressionEditsIntentError::BeforeMismatch)
    ));
}

#[test]
fn native_and_proof_pools_remain_separate_and_final_cancellation_refuses() {
    let before = raw::two_projections();
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
fn pending_committed_and_torn_marker_fixed_sessions_recover_both_edits() {
    let before = raw::two_projections();
    let after = expected_after(&before);
    let wire = literal(&before);
    for state in 0..4 {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = plan(&before)
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
                target.bytes.get_mut().copy_from_slice(&after);
            }
        }

        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_query_expression_edits_recovery(
            checked,
            decoded(&wire),
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
        qualify_native_query_expression_edits_recovery(
            checked,
            decoded(&wire),
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
fn plausible_partial_after_and_stale_target_never_gain_native_authority() {
    let before = raw::two_projections();
    let partial_after = raw::replace_at(&before, 4, "7000");
    let wire = literal(&before);
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut journal,
            &before,
            &partial_after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = target.writes;
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_query_expression_edits_recovery(
            checked,
            decoded(&wire),
            SCOPE,
            &mut budget(),
            &|| false
        ),
        Err(QueryExpressionEditsIntentError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    assert_eq!(target.writes, writes);
    assert_eq!(target.bytes.get_ref(), &before);

    let mut stale = before.clone();
    stale[0] ^= 1;
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
fn exact_decode_allocation_work_and_terminal_cancellation_are_enforced() {
    let before = raw::two_projections();
    let wire = literal(&before);
    let mut measured = budget();
    let polls = Cell::new(0);
    NativeQueryExpressionEditsIntent::decode(&wire, &mut measured, &|| {
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
                NativeQueryExpressionEditsIntent::decode(&wire, &mut limits, &|| false).is_ok(),
                admitted
            );
        }
    }
    let seen = Cell::new(0);
    assert!(
        NativeQueryExpressionEditsIntent::decode(&wire, &mut budget(), &|| {
            seen.set(seen.get() + 1);
            seen.get() == polls.get()
        })
        .is_err()
    );
}

#[test]
fn exact_noop_keeps_native_reconstruction_and_original_source_guards() {
    let before = raw::two_projections();
    let request = [QueryFragmentExpressionEdit {
        attribute: QueryAttribute::Column,
        order: [0, 0, 0, 2],
        expected: "99",
        replacement: "99",
    }];
    let reviewed = image(&before)
        .plan_recoverable_query_expression_edits("Saved", &request, &mut budget(), &|| false)
        .unwrap();
    assert!(!reviewed.changed());
    let fresh = reviewed
        .requalify_with_budgets(image(&before), &mut budget(), &mut budget(), &|| false)
        .unwrap();
    assert_eq!(fresh.after_bytes(), before);

    let mut opaque = before;
    let end = 4 * 4096
        + u16::from_le_bytes(opaque[4 * 4096 + 28..4 * 4096 + 30].try_into().unwrap()) as usize;
    opaque[end - 2] |= 0x40;
    assert!(
        image(&opaque)
            .plan_recoverable_query_expression_edits("Saved", &request, &mut budget(), &|| false)
            .is_err()
    );
}

#[test]
fn where_tag_decodes_as_data_but_cannot_retarget_a_projection_during_recovery() {
    let before = raw::two_projections();
    let mut wire = literal(&before);
    wire[85] = 8;
    let intent = decoded(&wire);
    assert_eq!(
        intent.edits()[0].as_request().attribute,
        QueryAttribute::Where
    );
    assert_eq!(intent.encode(&mut budget(), &|| false).unwrap(), wire);

    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        plan(&before)
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap(),
    );
    let writes = target.writes;
    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_query_expression_edits_recovery(
            checked,
            intent,
            SCOPE,
            &mut budget(),
            &|| false
        ),
        Err(QueryExpressionEditsIntentError::Expression(
            QueryExpressionEditsError::Definition
        ))
    ));
    assert_eq!(target.writes, writes);
    assert_eq!(target.bytes.get_ref(), &before);
}
