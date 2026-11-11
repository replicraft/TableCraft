//! Fresh V3 sessions preserve sparse logical identities and exact independent trees.
use super::{Error, Intent, budget, literal};
use crate::{NativeImage, RecoverableNativeFreshIndexCreatePlan, SecondaryIndexCreate};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::secondary_index::normal::{self as raw, history},
    support::io::journal::MemoryStorage,
};

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn wire(case: &raw::Source) -> Vec<u8> {
    literal(
        &case.before,
        [case.table, case.column, case.index_name],
        case.target_definition,
        case.descending,
    )
}
fn original(case: &raw::Source) -> Intent {
    Intent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}
fn plan(case: &raw::Source) -> RecoverableNativeFreshIndexCreatePlan {
    image(&case.before, 30810)
        .plan_recoverable_retained_fresh_long_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, case.descending),
            &mut budget(),
            &|| false,
        )
        .unwrap()
}
fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing completeimage byte"
    );
}

#[test]
fn fresh_logical_original_roundtrips_exact_history_and_old_constructor_remains_closed() {
    for descending in [false, true] {
        let case = history::source_with_direction(descending);
        let captured = plan(&case);
        same(captured.before_bytes(), &case.before);
        same(captured.after_bytes(), &case.expected);
        assert_eq!(
            captured.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        assert_eq!(captured.intent().logical_index_number(), 3);
        assert_eq!(captured.intent().physical_index_number(), 2);
        assert_eq!(captured.intent().native_number(), 4);
        assert_eq!(captured.intent().column_id(), 4);
        assert!(
            image(&case.before, 30811)
                .plan_recoverable_retained_long_index_creation(
                    case.table,
                    &SecondaryIndexCreate::new(case.index_name, case.column, case.descending),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "TCIC1 cannot silently enter the freshlogical source language"
        );
        let shared = captured.after_shared_bytes();
        assert_eq!(shared.as_ptr(), captured.after_bytes().as_ptr());
        let rebuilt = captured
            .requalify_with_budgets(
                image(&case.before, 30812),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(rebuilt.after_bytes(), &case.expected);
        drop(rebuilt);
        same(&shared, &case.expected);
    }
}

#[test]
fn fresh_history_pending_mixed_committed_and_settled_keep_same_v3_owner() {
    for descending in [false, true] {
        let case = history::source_with_direction(descending);
        for state in 0..3 {
            let mut target = MemoryStorage::new(case.before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan(&case)
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if state == 2 {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                if state == 1 {
                    *target.bytes.get_mut() = case.expected.to_vec();
                    for (page, old) in case.before.as_chunks::<4096>().0.iter().enumerate() {
                        if page % 2 == 0 {
                            target.bytes.get_mut()[page * 4096..(page + 1) * 4096]
                                .copy_from_slice(old);
                        }
                    }
                }
            }
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let qualified = original(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30820),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                qualified.state(),
                if state == 2 {
                    ResizableState::Committed
                } else {
                    ResizableState::Pending
                }
            );
            assert_eq!(
                qualified
                    .recover_retained(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                if state == 2 {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            same(
                target.bytes.get_ref(),
                if state == 2 {
                    &case.expected
                } else {
                    &case.before
                },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            original(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30821),
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
fn stale_foreign_after_and_native_or_new_index_identity_tampering_grant_no_effects() {
    let case = history::source();
    let mut stale = case.before.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&case)
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    same(target.bytes.get_ref(), &stale);
    for offset in [14, 24, 56, 60, 64, 66, 76, 80] {
        let mut wrong = wire(&case);
        if offset == 60 {
            let other = if case.target_definition == 1 {
                2_u32
            } else {
                1_u32
            };
            wrong[60..64].copy_from_slice(&other.to_le_bytes());
        } else {
            wrong[offset] ^= 1;
        }
        let intent = Intent::decode(&wrong, &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("canonical identity tamper offset {offset}: {error}"));
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &case.before,
                &case.expected,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            intent
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30830),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "tamperoffset {offset}"
        );
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &case.before);
    }
    for offset in [
        3 * 4096 + 3000,
        case.target_definition as usize * 4096 + 3000,
        case.root as usize * 4096 + 4000,
        case.map_host as usize * 4096 + 4000,
    ] {
        let mut forged = case.expected.to_vec();
        forged[offset] ^= 1;
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &case.before,
                &forged,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        assert!(
            original(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(30831),
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert_eq!(target.writes, 0);
    }
    assert!(matches!(
        original(&case).verify_before(&stale, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
}

#[test]
fn fresh_original_cannot_use_v1_v2_or_unpaid_requalification() {
    let case = history::source();
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.before, 30840),
                &mut budget(),
                &mut tablecraft_core::limits::ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    for version in [1, 2] {
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        if version == 1 {
            assert!(
                journal::prepare_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &case.expected,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
        } else {
            drop(
                journal::prepare_resizable_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &case.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let writes = journal_file.writes;
            assert!(journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).is_err());
            assert_eq!(journal_file.writes, writes);
        }
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &case.before);
    }
}
