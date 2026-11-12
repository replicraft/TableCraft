//! Cold numeric reconstruction uses original source facts and independent full images.
use super::{Error, Facts, Intent, budget, literal};
use crate::{NativeImage, RecoverableNativeNumericIndexCreatePlan, SecondaryIndexCreate};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::secondary_index::numeric::{self as raw, Kind, Population},
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
        Facts {
            table: case.target_definition as i32,
            definition: case.target_definition,
            field: 0,
            stored: 0,
            logical: 1,
            physical: 1,
            native_type: case.kind.native(),
        },
        case.descending,
    )
}

fn original(case: &raw::Source) -> Intent {
    Intent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

fn plan(case: &raw::Source) -> RecoverableNativeNumericIndexCreatePlan {
    image(&case.before, 32110)
        .plan_recoverable_retained_numeric_index_creation(
            case.table,
            &SecondaryIndexCreate::new(case.index_name, case.column, case.descending),
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(left, right)| left != right),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn all_numeric_originals_match_literal_wire_and_full_raw_images_without_old_intent_widening() {
    for kind in raw::KINDS {
        for population in [Population::Empty, Population::NullableDuplicate] {
            for descending in [false, true] {
                let case = raw::source(kind, population, descending);
                let captured = plan(&case);
                same(captured.before_bytes(), &case.before);
                same(captured.after_bytes(), &case.expected);
                assert_eq!(
                    captured.intent().encode(&mut budget(), &|| false).unwrap(),
                    wire(&case)
                );
                assert_eq!(captured.intent().native_type(), kind.native());
                assert_eq!(captured.intent().native_number(), 0);
                assert_eq!(captured.intent().column_id(), 0);
                assert_eq!(captured.intent().logical_index_number(), 1);
                assert_eq!(captured.intent().physical_index_number(), 1);
                assert_eq!(captured.indexed_rows(), case.rows.len());
                let shared = captured.after_shared_bytes();
                assert_eq!(shared.as_ptr(), captured.after_bytes().as_ptr());
                drop(captured);
                same(&shared, &case.expected);
                if kind != Kind::Int32 {
                    for fresh in [false, true] {
                        let source = image(&case.before, 32111);
                        let request =
                            SecondaryIndexCreate::new(case.index_name, case.column, descending);
                        let refused = if fresh {
                            source
                                .plan_recoverable_retained_fresh_long_index_creation(
                                    case.table,
                                    &request,
                                    &mut budget(),
                                    &|| false,
                                )
                                .is_err()
                        } else {
                            source
                                .plan_recoverable_retained_long_index_creation(
                                    case.table,
                                    &request,
                                    &mut budget(),
                                    &|| false,
                                )
                                .is_err()
                        };
                        assert!(refused, "old LONG original widened to {kind:?}");
                    }
                }
            }
        }
    }
    let case = raw::source(Kind::Double, Population::NullableDuplicate, true);
    let shared = plan(&case).after_shared_bytes();
    let rebuilt = plan(&case)
        .requalify_with_budgets(
            image(&case.before, 32112),
            &mut budget(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    same(rebuilt.after_bytes(), &case.expected);
    drop(rebuilt);
    same(&shared, &case.expected);
}

#[test]
fn numeric_pending_mixed_committed_and_settled_recovery_keep_the_same_v3_owner() {
    for (kind, population) in [
        (Kind::Boolean, Population::NullableDuplicate),
        (Kind::Double, Population::NullableDuplicate),
        (Kind::Int64, Population::ThreeLeaves),
    ] {
        for descending in [false, true] {
            let case = raw::source(kind, population, descending);
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
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                let qualified = original(&case)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(32120),
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
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                original(&case)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(32121),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    target.writes, writes,
                    "settled confirmation must not replay writes"
                );
            }
        }
    }
}

fn reject_original(case: &raw::Source, intent: Intent, after: &[u8]) {
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
    let journal_writes = journal_file.writes;
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        intent
            .qualify_retained_recovery(checked, CatalogScope::new(32130), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, journal_writes);
    same(target.bytes.get_ref(), &case.before);
}

#[test]
fn scalar_native_allocated_identity_request_and_whole_after_tampering_grant_no_effects() {
    let case = raw::source(Kind::Int32, Population::NullableDuplicate, false);
    for offset in [14, 24, 56, 60, 64, 66, 76, 80, 84] {
        let mut changed = wire(&case);
        match offset {
            60 => {
                let other = if case.target_definition == 1 {
                    2_u32
                } else {
                    1_u32
                };
                changed[60..64].copy_from_slice(&other.to_le_bytes());
            }
            84 => changed[84] = 19,
            _ => changed[offset] ^= 1,
        }
        let intent = Intent::decode(&changed, &mut budget(), &|| false)
            .unwrap_or_else(|error| panic!("canonical identity tamper at {offset}: {error}"));
        reject_original(&case, intent, &case.expected);
    }
    for start in [
        88 + case.table.len(),
        88 + case.table.len() + case.column.len(),
    ] {
        let mut changed = wire(&case);
        changed[start] = b'X';
        reject_original(
            &case,
            Intent::decode(&changed, &mut budget(), &|| false).unwrap(),
            &case.expected,
        );
    }
    for offset in [
        3 * 4096 + 3000,
        case.target_definition as usize * 4096 + 3000,
        case.root as usize * 4096 + 4000,
        case.map_host as usize * 4096 + 4000,
    ] {
        let mut forged = case.expected.to_vec();
        forged[offset] ^= 1;
        reject_original(&case, original(&case), &forged);
    }
    let mut stale = case.before.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    assert!(matches!(
        original(&case).verify_before(&stale, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&case)
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    same(target.bytes.get_ref(), &stale);
}

#[test]
fn numeric_original_requires_paid_uncancelled_fresh_source_and_v3_framing() {
    let case = raw::source(Kind::Double, Population::NullableDuplicate, false);
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.before, 32140),
                &mut budget(),
                &mut ResourceBudget::new(0, 0, 0),
                &|| false
            )
            .is_err()
    );
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.before, 32141),
                &mut budget(),
                &mut budget(),
                &|| true
            )
            .is_err()
    );
    let mut stale = case.before.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    assert!(
        plan(&case)
            .requalify_with_budgets(image(&stale, 32142), &mut budget(), &mut budget(), &|| {
                false
            })
            .is_err()
    );
    for cancelled in [false, true] {
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
        let journal_writes = journal_file.writes;
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let mut unpaid = if cancelled {
            budget()
        } else {
            ResourceBudget::new(0, 0, 0)
        };
        assert!(
            original(&case)
                .qualify_retained_recovery(checked, CatalogScope::new(32143), &mut unpaid, &|| {
                    cancelled
                },)
                .is_err()
        );
        assert_eq!((target.writes, journal_file.writes), (0, journal_writes));
        same(target.bytes.get_ref(), &case.before);
    }
    for version in [1, 2] {
        let mut target = MemoryStorage::new(case.before.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        if version == 1 {
            let mut fixed = case.before.to_vec();
            fixed[7000] ^= 1;
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut journal_file,
                    &case.before,
                    &fixed,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
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
        }
        let writes = journal_file.writes;
        assert!(journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).is_err());
        assert_eq!((target.writes, journal_file.writes), (0, writes));
        same(target.bytes.get_ref(), &case.before);
    }
}
