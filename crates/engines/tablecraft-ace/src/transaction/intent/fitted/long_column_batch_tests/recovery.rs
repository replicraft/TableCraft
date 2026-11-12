//! Reconstruct atomic batches from one original, never a sequence of cell saves.
use super::{Cell, budget, groups, literal};
use crate::{
    NativeImage, NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent,
    NativeUpdateProfile, RecoverableNativeEditPlan, qualify_native_allocation_recovery,
    qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::long_column_batch as raw,
    support::io::journal::MemoryStorage,
};

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn cells(case: &raw::Source) -> Vec<Cell> {
    case.cells
        .iter()
        .map(|cell| Cell {
            row: cell.row,
            expected: cell.expected,
            value: cell.value,
        })
        .collect()
}

fn wire(case: &raw::Source) -> Vec<u8> {
    literal(&case.before, case.table, case.column, &cells(case))
}

fn decode(case: &raw::Source) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

fn plan(case: &raw::Source) -> RecoverableNativeEditPlan {
    image(&case.before, 32520)
        .plan_recoverable_retained_long_column_batch_updates(
            case.table,
            groups(case.column, &cells(case)),
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
        "first differing complete image byte"
    );
}

#[test]
fn batch_original_matches_literal_source_and_complete_effect_in_fresh_scopes() {
    for case in [
        raw::two_rows(),
        raw::three_rows(),
        raw::forwarded(),
        raw::cross_page(),
        raw::materializing(),
    ] {
        let captured = plan(&case);
        assert_eq!(
            captured.intent().profile(),
            NativeUpdateProfile::Ace16RetainedLongColumnBatch
        );
        assert_eq!(
            captured.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        same(captured.before_bytes(), &case.before);
        same(captured.after_bytes(), &case.expected);
        let rebuilt = captured
            .requalify_with_budgets(
                image(&case.before, 32521),
                &mut budget(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        same(rebuilt.after_bytes(), &case.expected);
        drop(rebuilt);
    }
}

#[test]
fn batch_v3_pending_mixed_committed_and_settled_states_preserve_atomic_identity() {
    let mut noop = raw::two_rows();
    noop.expected = noop.before.to_vec();
    for cell in &mut noop.cells {
        cell.value = cell.expected;
    }
    for case in [
        raw::two_rows(),
        raw::cross_page(),
        raw::materializing(),
        noop,
    ] {
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
            let qualified = decode(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(32522),
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
            decode(&case)
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(32523),
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

fn reject(case: &raw::Source, original: NativeUpdateIntent, after: &[u8]) {
    let mut target = MemoryStorage::new(case.before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &case.before,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = journal_file.writes;
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        original
            .qualify_retained_recovery(checked, CatalogScope::new(32524), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, writes));
    same(target.bytes.get_ref(), &case.before);
}

#[test]
fn source_late_expected_wrong_request_foreign_after_and_old_protocols_grant_no_effects() {
    let case = raw::two_rows();
    let mut wrong = wire(&case);
    wrong[24] ^= 1;
    reject(
        &case,
        NativeUpdateIntent::decode(&wrong, &mut budget(), &|| false).unwrap(),
        &case.expected,
    );
    let mut last = cells(&case);
    last[1].expected = Some(999);
    let wrong = literal(&case.before, case.table, case.column, &last);
    reject(
        &case,
        NativeUpdateIntent::decode(&wrong, &mut budget(), &|| false).unwrap(),
        &case.expected,
    );
    let wrong = literal(&case.before, case.table, "Other", &cells(&case));
    reject(
        &case,
        NativeUpdateIntent::decode(&wrong, &mut budget(), &|| false).unwrap(),
        &case.expected,
    );
    for offset in [
        3 * 4096 + 3000,
        case.definition as usize * 4096 + 3000,
        (case.definition as usize + 1) * 4096 + 3000,
    ] {
        let mut after = case.expected.to_vec();
        after[offset] ^= 1;
        reject(&case, decode(&case), &after);
    }
    // TCNI17 data historically accepts multiple rows; its native one-cell
    // authority must remain closed after a deliberate data-only retag.
    let mut old = wire(&case);
    old[..8].copy_from_slice(b"TCNI0017");
    old[12..14].copy_from_slice(&17_u16.to_le_bytes());
    reject(
        &case,
        NativeUpdateIntent::decode(&old, &mut budget(), &|| false).unwrap(),
        &case.expected,
    );
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
    assert!(
        plan(&case)
            .requalify_with_budgets(image(&stale, 32525), &mut budget(), &mut budget(), &|| {
                false
            })
            .is_err()
    );
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.before, 32526),
                &mut budget(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.before, 32527),
                &mut budget(),
                &mut budget(),
                &|| true
            )
            .is_err()
    );
    for version in [1, 2] {
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        if version == 1 {
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &case.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_update_recovery(
                    checked,
                    decode(&case),
                    CatalogScope::new(32528),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::Intent(
                    NativeIntentError::UnsupportedProfile
                ))
            ));
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
            let checked = journal::inspect_v2(&mut journal_file, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_allocation_recovery(
                    checked,
                    decode(&case),
                    CatalogScope::new(32529),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::Intent(
                    NativeIntentError::UnsupportedProfile
                ))
            ));
            assert_eq!(journal_file.writes, writes);
            assert!(journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).is_err());
        }
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &case.before);
    }
}
