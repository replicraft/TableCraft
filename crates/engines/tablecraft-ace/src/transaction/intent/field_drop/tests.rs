//! Independent request bytes and native reconstruction through the real V1 owner.
use super::*;
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_io::journal::{self, RecoveryOutcome, V1State, inspect_v1};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw,
    support::{bytes::hex, io::journal::MemoryStorage},
};

const SCOPE: CatalogScope = CatalogScope::new(993);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(before: &[u8], scope: CatalogScope) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), scope, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8], column: &str) -> RecoverableFieldDropPlan {
    image(before, SCOPE)
        .plan_recoverable_saved_field_drop("Table1", column, &mut budget(), &|| false)
        .unwrap()
}

fn decode(bytes: &[u8]) -> NativeFieldDropIntent {
    NativeFieldDropIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}

fn literal(before: &[u8], column: &str) -> Vec<u8> {
    // Independent 76-byte header. Both observed targets have native number 1,
    // stored creation ID 1 and table/root 7. There is no serialized caller scope.
    let length = 76 + 6 + column.len();
    let mut output = b"TCFD0001".to_vec();
    output.extend((length as u32).to_le_bytes());
    output.extend(1_u16.to_le_bytes());
    output.extend(0_u16.to_le_bytes());
    output.extend((before.len() as u64).to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((before.len() as u64).to_le_bytes());
    binding.update(before);
    output.extend(binding.finalize());
    output.extend(7_i32.to_le_bytes());
    output.extend(7_u32.to_le_bytes());
    output.extend(1_u16.to_le_bytes());
    output.extend(1_u16.to_le_bytes());
    output.extend(6_u16.to_le_bytes());
    output.extend((column.len() as u16).to_le_bytes());
    output.extend(0_u32.to_le_bytes());
    output.extend(b"Table1");
    output.extend(column.as_bytes());
    output
}

#[test]
fn independent_wire_and_both_complete_raw_afters_survive_caller_scope_rebinding() {
    for (case, column, expected) in [
        (
            raw::SourceCase::NumberHistory,
            "Score",
            "6c08831dfe4d305064837d93a8d1942a1a87b691ca80e4834a130c202c72923b",
        ),
        (
            raw::SourceCase::MixedTextHistory,
            "Label",
            "d9432f5edadc0bfcd60551f747be5e6ac630585f5cfa78fffdced6f7e875b935",
        ),
    ] {
        let before = raw::source(case);
        let expected_after =
            raw::remove_nonprimary(&before, column, raw::Variant::AppendedGroupRemoved);
        let reviewed = plan(&before, column);
        let old_object = reviewed.identity().object_id();
        let encoded = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(encoded, literal(&before, column));
        assert_eq!(reviewed.before_bytes(), before);
        assert_eq!(reviewed.after_bytes(), expected_after);
        assert_eq!(hex(&Sha256::digest(reviewed.after_bytes())), expected);
        assert_eq!(
            decode(&encoded).encode(&mut budget(), &|| false).unwrap(),
            encoded
        );

        let fresh = reviewed
            .requalify_with_budgets(
                image(&before, CatalogScope::new(994)),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_ne!(fresh.identity().object_id(), old_object);
        assert_eq!(fresh.intent().column_number(), 1);
        assert_eq!(fresh.intent().column_id(), 1);
        assert_eq!(fresh.identity().column_number(), 1);
        assert_eq!(fresh.table(), "Table1");
        assert_eq!(fresh.column(), column);
        assert_eq!(fresh.before_bytes().len(), fresh.after_bytes().len());
        assert_eq!(fresh.after_bytes(), expected_after);
    }
}

#[test]
fn closed_decode_rejects_versions_shapes_and_native_rename_before_owned_text() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let good = literal(&before, "Score");
    assert!(matches!(
        crate::NativeFieldRenameIntent::decode(&good, &mut budget(), &|| false),
        Err(crate::FieldRenameIntentError::InvalidRecord)
    ));
    let mut cases = (0..good.len())
        .map(|end| good[..end].to_vec())
        .collect::<Vec<_>>();
    for offset in [0, 7, 8, 12, 14, 60, 68, 70, 72, 76, 82] {
        let mut changed = good.clone();
        changed[offset] ^= 255;
        cases.push(changed);
    }
    let mut protected = good.clone();
    protected[64..66].copy_from_slice(&0_u16.to_le_bytes());
    cases.push(protected);
    let mut rename = good.clone();
    rename[..8].copy_from_slice(b"TCFR0001");
    cases.push(rename);
    cases.push(literal(&before, "Scoré"));
    cases.push(literal(&before, "Bad!"));
    cases.push(literal(&before, ""));
    for bytes in cases {
        let mut work = budget();
        let bytes_before = work.remaining(Resource::Bytes);
        assert!(matches!(
            NativeFieldDropIntent::decode(&bytes, &mut work, &|| false),
            Err(FieldDropIntentError::InvalidRecord)
        ));
        assert_eq!(work.remaining(Resource::Bytes), bytes_before);
    }
}

#[test]
fn changed_original_binding_native_identity_and_missing_field_never_gain_authority() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let good = literal(&before, "Score");
    let mut changed = before.clone();
    changed[4096 + 3000] ^= 1;
    assert!(matches!(
        decode(&good).verify_before(&changed, &mut budget(), &|| false),
        Err(FieldDropIntentError::BeforeMismatch)
    ));
    for component in 0..4 {
        let mut intent = decode(&good);
        match component {
            0 => intent.identity.native_table += 1,
            1 => intent.identity.definition += 1,
            2 => intent.identity.number += 1,
            3 => intent.identity.column_id += 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            intent.reconstruct(image(&before, SCOPE), &mut budget(), &|| false),
            Err(FieldDropIntentError::IdentityMismatch)
        ));
    }
    let missing = decode(&literal(&before, "Wrong"));
    assert!(matches!(
        missing.reconstruct(image(&before, SCOPE), &mut budget(), &|| false),
        Err(FieldDropIntentError::Drop(_))
    ));
}

#[test]
fn same_checked_v1_covers_pending_committed_torn_marker_and_settled_without_replay() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let request = literal(&before, "Score");
    for mode in 0..4 {
        let reviewed = plan(&before, "Score");
        let after = reviewed.after_bytes().to_vec();
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        let prepared = reviewed
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        if mode == 1 || mode == 3 {
            prepared.commit(&mut budget(), &|| false).unwrap();
            if mode == 3 {
                let partial = journal.bytes.get_ref().len() - 20;
                journal.bytes.get_mut().truncate(partial);
            }
        } else {
            drop(prepared);
            if mode == 2 {
                target.bytes.get_mut().copy_from_slice(&after);
            }
        }
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_field_drop_recovery(
            checked,
            decode(&request),
            SCOPE,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            qualified.state(),
            if mode == 1 {
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
            if mode == 1 {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            }
        );
        assert_eq!(
            target.bytes.get_ref(),
            if mode == 1 { &after } else { &before }
        );

        let writes = target.writes;
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        qualify_native_field_drop_recovery(
            checked,
            decode(&request),
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
fn checked_journal_with_patched_after_or_matching_digest_bad_source_is_not_authority() {
    let original = raw::source(raw::SourceCase::NumberHistory);
    let expected = plan(&original, "Score").after_bytes().to_vec();
    for bad_source in [false, true] {
        let mut before = original.clone();
        let mut after = expected.clone();
        if bad_source {
            before[4096 + 3000] ^= 1;
        } else {
            after[7 * 4096 + 3000] ^= 1;
        }
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
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let result = qualify_native_field_drop_recovery(
            checked,
            decode(&literal(&before, "Score")),
            SCOPE,
            &mut budget(),
            &|| false,
        );
        if bad_source {
            assert!(matches!(result, Err(FieldDropIntentError::Drop(_))));
        } else {
            assert!(matches!(
                result,
                Err(FieldDropIntentError::Proof(
                    NativeRecoveryQualificationError::ImageMismatch
                ))
            ));
        }
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}

#[test]
fn split_pools_terminal_cancellation_and_stale_target_preserve_original_resources() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    for native_empty in [false, true] {
        let empty = || ResourceBudget::new(0, 0, 0);
        let mut native = if native_empty { empty() } else { budget() };
        let mut proof = if native_empty { budget() } else { empty() };
        let result = plan(&before, "Score").requalify_with_budgets(
            image(&before, SCOPE),
            &mut native,
            &mut proof,
            &|| false,
        );
        if native_empty {
            assert!(matches!(result, Err(FieldDropIntentError::Drop(_))));
        } else {
            assert!(matches!(
                result,
                Err(FieldDropIntentError::Proof(
                    NativeRecoveryQualificationError::Inspect(InspectError::Limit(_))
                ))
            ));
        }
    }
    let polls = Cell::new(0);
    plan(&before, "Score")
        .requalify_with_budgets(image(&before, SCOPE), &mut budget(), &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let observed = Cell::new(0);
    assert!(matches!(
        plan(&before, "Score").requalify_with_budgets(
            image(&before, SCOPE),
            &mut budget(),
            &mut budget(),
            &|| {
                observed.set(observed.get() + 1);
                observed.get() == polls.get()
            },
        ),
        Err(FieldDropIntentError::Inspect(InspectError::Cancelled))
    ));

    let mut stale = before.clone();
    stale[4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    assert!(matches!(
        plan(&before, "Score").prepare(&mut target, &mut journal, &mut budget(), &|| false),
        Err(journal::JournalError::TargetMismatch)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
    assert_eq!(target.bytes.get_ref(), &stale);
}

#[test]
fn recovery_uses_one_cumulative_pool_and_initial_cancellation_never_writes() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let request = literal(&before, "Score");
    for cancelled in [false, true] {
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        drop(
            plan(&before, "Score")
                .prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap(),
        );
        let journal_writes = journal.writes;
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let mut work = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        let result = qualify_native_field_drop_recovery(
            checked,
            decode(&request),
            SCOPE,
            &mut work,
            &|| cancelled,
        );
        if cancelled {
            assert!(matches!(
                result,
                Err(FieldDropIntentError::Inspect(InspectError::Cancelled))
            ));
        } else {
            assert!(matches!(
                result,
                Err(FieldDropIntentError::Inspect(InspectError::Limit(_)))
            ));
        }
        assert_eq!(target.writes, 0);
        assert_eq!(journal.writes, journal_writes);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}
