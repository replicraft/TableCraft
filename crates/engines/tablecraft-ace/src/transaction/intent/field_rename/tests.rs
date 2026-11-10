//! Independent request framing and real retained-journal reconstruction boundaries.
mod populated;
use super::*;
use sha2::{Digest, Sha256};
use std::{cell::Cell, io::Cursor};
use tablecraft_io::journal::{self, RecoveryOutcome, V1State, inspect_v1};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_created_number::mixed::after_off as raw,
    support::{bytes::hex, io::journal::MemoryStorage},
};

const SCOPE: CatalogScope = CatalogScope::new(989);

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 5_000_000)
}

fn image(before: &[u8], scope: CatalogScope) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(before), scope, &mut budget(), &|| false).unwrap()
}

fn plan(before: &[u8], original: &str, replacement: &str) -> RecoverableFieldRenamePlan {
    image(before, SCOPE)
        .plan_recoverable_same_width_field_rename(
            "Table1",
            original,
            replacement,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn decode(bytes: &[u8]) -> NativeFieldRenameIntent {
    NativeFieldRenameIntent::decode(bytes, &mut budget(), &|| false).unwrap()
}

fn literal(before: &[u8], original: &str, replacement: &str) -> Vec<u8> {
    // Independent 80-byte wire. This first field has number 1 and stored ID 1,
    // root/native table 7, and no serialized caller-scoped ObjectId or spans.
    let length = 80 + 6 + original.len() + replacement.len();
    let mut bytes = b"TCFR0001".to_vec();
    bytes.extend((length as u32).to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    bytes.extend((before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    bytes.extend(digest.finalize());
    bytes.extend(7_i32.to_le_bytes());
    bytes.extend(7_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend((original.len() as u16).to_le_bytes());
    bytes.extend(6_u16.to_le_bytes());
    bytes.extend((original.len() as u16).to_le_bytes());
    bytes.extend((replacement.len() as u16).to_le_bytes());
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend(b"Table1");
    bytes.extend(original.as_bytes());
    bytes.extend(replacement.as_bytes());
    bytes
}

#[test]
fn independent_wire_pins_native_request_and_complete_raw_after_across_scopes() {
    for (case, original, replacement, expected) in [
        (
            raw::SourceCase::Number,
            "Score",
            "Total",
            "235a57e709bb65bc6a1d0cc6483c2b204f1d3b78924108ff20e96a6671ba263d",
        ),
        (
            raw::SourceCase::Mixed,
            "Label",
            "Title",
            "7604ae8af88cbcaf1be537e03cfc65c6f8095ed20a8f4c1113359c7d176ddfdc",
        ),
    ] {
        let before = raw::source(case);
        let reviewed = plan(&before, original, replacement);
        let encoded = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
        assert_eq!(encoded, literal(&before, original, replacement));
        assert_eq!(
            decode(&encoded).encode(&mut budget(), &|| false).unwrap(),
            encoded
        );
        let fresh = reviewed
            .requalify_with_budgets(
                image(&before, CatalogScope::new(990)),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(fresh.intent().column_number(), 1);
        assert_eq!(fresh.intent().column_id(), 1);
        assert_eq!(fresh.identity().column_number(), 1);
        assert_eq!(fresh.table(), "Table1");
        assert_eq!(fresh.column(), original);
        assert_eq!(fresh.new_name(), replacement);
        assert!(fresh.changed());
        assert_eq!(hex(&Sha256::digest(fresh.after_bytes())), expected);
    }
}

#[test]
fn closed_codec_refuses_malformed_borrowed_requests_before_any_string_allocation() {
    let before = raw::source(raw::SourceCase::Number);
    let good = literal(&before, "Score", "Total");
    assert!(matches!(
        crate::NativeTableRenameIntent::decode(&good, &mut budget(), &|| false),
        Err(crate::TableRenameIntentError::InvalidRecord)
    ));
    let mut cases = (0..good.len())
        .map(|end| good[..end].to_vec())
        .collect::<Vec<_>>();
    for offset in [0, 7, 8, 12, 14, 60, 68, 70, 72, 74, 76, 80, 86] {
        let mut changed = good.clone();
        changed[offset] ^= 255;
        cases.push(changed);
    }
    cases.push(literal(&before, "Score", "To!al"));
    cases.push(literal(&before, "Score", "Longer"));
    cases.push(literal(&before, "Score", "Scoré"));
    for bytes in cases {
        let mut work = budget();
        let bytes_before = work.remaining(Resource::Bytes);
        assert!(matches!(
            NativeFieldRenameIntent::decode(&bytes, &mut work, &|| false),
            Err(FieldRenameIntentError::InvalidRecord)
        ));
        assert_eq!(work.remaining(Resource::Bytes), bytes_before);
    }
}

#[test]
fn changed_binding_stored_id_number_and_original_name_never_gain_native_authority() {
    let before = raw::source(raw::SourceCase::Number);
    let good = literal(&before, "Score", "Total");
    let mut changed_source = before.clone();
    changed_source[4096 + 3000] ^= 1;
    assert!(matches!(
        decode(&good).verify_before(&changed_source, &mut budget(), &|| false),
        Err(FieldRenameIntentError::BeforeMismatch)
    ));
    for offset in [56, 60, 64, 66] {
        let mut request = decode(&good);
        match offset {
            56 => request.identity.native_table += 1,
            60 => request.identity.definition += 1,
            64 => request.identity.number += 1,
            66 => request.identity.column_id += 1,
            _ => unreachable!(),
        }
        assert!(matches!(
            request.reconstruct(image(&before, SCOPE), &mut budget(), &|| false),
            Err(FieldRenameIntentError::IdentityMismatch)
        ));
    }
    let unknown_original = decode(&literal(&before, "Wrong", "Total"));
    assert!(matches!(
        unknown_original.reconstruct(image(&before, SCOPE), &mut budget(), &|| false),
        Err(FieldRenameIntentError::Rename(FieldRenameError::Source))
    ));
}

#[test]
fn exact_noop_and_case_only_requests_remain_distinct_and_fully_qualified() {
    let before = raw::source(raw::SourceCase::Number);
    for replacement in ["Score", "score"] {
        let reviewed = plan(&before, "Score", replacement);
        assert_eq!(reviewed.changed(), replacement != "Score");
        assert_eq!(
            reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
            literal(&before, "Score", replacement)
        );
        let fresh = reviewed
            .requalify_with_budgets(image(&before, SCOPE), &mut budget(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(fresh.changed(), replacement != "Score");
    }
    let mut altered = before;
    altered[7 * 4096 + 3000] = 1;
    assert!(matches!(
        image(&altered, SCOPE).plan_recoverable_same_width_field_rename(
            "Table1",
            "Score",
            "Score",
            &mut budget(),
            &|| false
        ),
        Err(FieldRenameIntentError::Rename(
            FieldRenameError::SourceProof(_)
        ))
    ));
}

#[test]
fn same_retained_v1_pending_committed_torn_and_settled_states_are_native_qualified() {
    let before = raw::source(raw::SourceCase::Number);
    let request = literal(&before, "Score", "Total");
    for mode in 0..4 {
        let reviewed = plan(&before, "Score", "Total");
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
        let qualified = qualify_native_field_rename_recovery(
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
        assert_eq!(
            qualified
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap(),
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
        qualify_native_field_rename_recovery(
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
fn syntactically_checked_patched_after_never_receives_recovery_authority() {
    let before = raw::source(raw::SourceCase::Number);
    let mut after = plan(&before, "Score", "Total").after_bytes().to_vec();
    after[7 * 4096 + 3000] ^= 1;
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
    assert!(matches!(
        qualify_native_field_rename_recovery(
            checked,
            decode(&literal(&before, "Score", "Total")),
            SCOPE,
            &mut budget(),
            &|| false
        ),
        Err(FieldRenameIntentError::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}

#[test]
fn independent_pool_and_terminal_cancellation_refusals_preserve_original_resources() {
    let before = raw::source(raw::SourceCase::Number);
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
        let result = plan(&before, "Score", "Total").requalify_with_budgets(
            image(&before, SCOPE),
            &mut native,
            &mut proof,
            &|| false,
        );
        if native_empty {
            assert!(matches!(
                result,
                Err(FieldRenameIntentError::Rename(FieldRenameError::Inspect(
                    InspectError::Limit(_)
                )))
            ));
        } else {
            assert!(matches!(
                result,
                Err(FieldRenameIntentError::Proof(
                    NativeRecoveryQualificationError::Inspect(InspectError::Limit(_))
                ))
            ));
        }
    }
    let polls = Cell::new(0);
    plan(&before, "Score", "Total")
        .requalify_with_budgets(image(&before, SCOPE), &mut budget(), &mut budget(), &|| {
            polls.set(polls.get() + 1);
            false
        })
        .unwrap();
    let observed = Cell::new(0);
    assert!(matches!(
        plan(&before, "Score", "Total").requalify_with_budgets(
            image(&before, SCOPE),
            &mut budget(),
            &mut budget(),
            &|| {
                observed.set(observed.get() + 1);
                observed.get() == polls.get()
            }
        ),
        Err(FieldRenameIntentError::Inspect(InspectError::Cancelled))
    ));
    assert!(matches!(
        NativeFieldRenameIntent::decode(
            &literal(&before, "Score", "Total"),
            &mut budget(),
            &|| true
        ),
        Err(FieldRenameIntentError::Inspect(InspectError::Cancelled))
    ));
}

#[test]
fn stale_target_refuses_before_journal_or_target_writes() {
    let before = raw::source(raw::SourceCase::Number);
    let mut stale = before.clone();
    stale[4096 + 3000] = 1;
    let mut target = MemoryStorage::new(stale.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    let result =
        plan(&before, "Score", "Total")
            .prepare(&mut target, &mut journal, &mut budget(), &|| false);
    assert!(matches!(result, Err(journal::JournalError::TargetMismatch)));
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
    assert_eq!(target.bytes.get_ref(), &stale);
}

mod fitting;
