//! Native reconstruction, checked V3 state and full independent UNIQUE images.
use super::{Error, Intent, budget, literal};
use crate::{NativeImage, RecoverableNativeUniqueLongIndexCreatePlan, UniqueLongIndexCreate};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::secondary_index::normal::{self as normal, unique_workflow as raw},
    support::io::journal::MemoryStorage,
};

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(31810),
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
        "first complete-image difference"
    );
}

fn source(descending: bool) -> normal::Source {
    raw::creation(descending, &[None, Some(-7), None, Some(0), Some(9)])
}

fn wire(source: &normal::Source) -> Vec<u8> {
    literal(
        &source.before,
        [source.table, source.column, source.index_name],
        source.target_definition,
        source.descending,
    )
}

fn plan(source: &normal::Source) -> RecoverableNativeUniqueLongIndexCreatePlan {
    image(&source.before)
        .plan_recoverable_retained_unique_long_index_creation(
            source.table,
            &UniqueLongIndexCreate::new(source.index_name, source.column, source.descending),
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn unique_creation_plan_matches_literal_original_shared_after_and_fresh_requalification() {
    for descending in [false, true] {
        let source = source(descending);
        let captured = plan(&source);
        same(captured.before_bytes(), &source.before);
        same(captured.after_bytes(), &source.expected);
        assert_eq!(
            captured.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&source)
        );
        assert_eq!(captured.indexed_rows(), source.rows.len());
        let shared = captured.after_shared_bytes();
        assert_eq!(shared.as_ptr(), captured.after_bytes().as_ptr());
        let rebuilt = captured
            .requalify_with_budgets(image(&source.before), &mut budget(), &mut budget(), &|| {
                false
            })
            .unwrap();
        same(rebuilt.after_bytes(), &source.expected);
        drop(rebuilt);
        same(&shared, &source.expected);
    }
}

#[test]
fn unique_creation_pending_mixed_committed_and_settled_keep_exact_checked_v3_state() {
    for descending in [false, true] {
        let source = source(descending);
        for state in 0..3 {
            let mut target = MemoryStorage::new(source.before.to_vec());
            let mut log = MemoryStorage::new(Vec::new());
            let prepared = plan(&source)
                .prepare_retained(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap();
            if state == 2 {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                if state == 1 {
                    *target.bytes.get_mut() = source.expected.to_vec();
                    for (page, before) in source.before.as_chunks::<4096>().0.iter().enumerate() {
                        if page % 2 == 0 {
                            target.bytes.get_mut()[page * 4096..(page + 1) * 4096]
                                .copy_from_slice(before);
                        }
                    }
                }
            }
            let checked = journal::inspect_v3(&mut log, &mut budget(), &|| false).unwrap();
            let qualified = Intent::decode(&wire(&source), &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(31811),
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
            let outcome = qualified
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                outcome,
                if state == 2 {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            same(
                target.bytes.get_ref(),
                if state == 2 {
                    &source.expected
                } else {
                    &source.before
                },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut log, &mut budget(), &|| false).unwrap();
            Intent::decode(&wire(&source), &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(31812),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                target.writes, writes,
                "settled confirmation never repeats schema effects"
            );
        }
    }
}

#[test]
fn unique_creation_stale_identity_wrong_request_and_foreign_after_cannot_write_target() {
    let source = source(false);
    let mut stale = source.before.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale.to_vec());
    let mut log = MemoryStorage::new(Vec::new());
    assert!(
        plan(&source)
            .prepare_retained(&mut target, &mut log, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, log.writes), (0, 0));
    same(target.bytes.get_ref(), &stale);
    assert!(matches!(
        Intent::decode(&wire(&source), &mut budget(), &|| false)
            .unwrap()
            .verify_before(&stale, &mut budget(), &|| false),
        Err(Error::BeforeMismatch)
    ));

    for offset in [
        24,
        56,
        60,
        64,
        66,
        76,
        80,
        84 + source.table.len() + source.column.len(),
    ] {
        let mut wrong = wire(&source);
        if offset == 60 {
            wrong[60..64].copy_from_slice(&1_u32.to_le_bytes());
        } else if offset >= 84 {
            wrong[offset] = b'X';
        } else {
            wrong[offset] ^= 1;
        }
        let original = Intent::decode(&wrong, &mut budget(), &|| false).unwrap();
        let mut target = MemoryStorage::new(source.before.to_vec());
        let mut log = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut log,
                &source.before,
                &source.expected,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut log, &mut budget(), &|| false).unwrap();
        assert!(
            original
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(31813),
                    &mut budget(),
                    &|| false
                )
                .is_err(),
            "identity offset {offset}"
        );
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &source.before);
    }
    for at in [3 * 4096 + 3000, source.root as usize * 4096 + 1000] {
        let mut foreign = source.expected.to_vec();
        foreign[at] ^= 1;
        let mut target = MemoryStorage::new(source.before.to_vec());
        let mut log = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut log,
                &source.before,
                &foreign,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut log, &mut budget(), &|| false).unwrap();
        let original = Intent::decode(&wire(&source), &mut budget(), &|| false).unwrap();
        assert!(
            original
                .qualify_retained_recovery(
                    checked,
                    CatalogScope::new(31814),
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &source.before);
    }
}

#[test]
fn unsupported_journal_versions_and_unpaid_or_cancelled_qualification_have_no_effects() {
    let source = source(false);
    for version in [1, 2, 3] {
        let mut target = MemoryStorage::new(source.before.to_vec());
        let mut log = MemoryStorage::new(Vec::new());
        if version == 1 {
            assert!(
                journal::prepare_expected(
                    &mut target,
                    &mut log,
                    &source.before,
                    &source.expected,
                    &mut budget(),
                    &|| false
                )
                .is_err()
            );
            assert_eq!((target.writes, log.writes), (0, 0));
            continue;
        } else if version == 2 {
            drop(
                journal::prepare_resizable_expected(
                    &mut target,
                    &mut log,
                    &source.before,
                    &source.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        } else {
            drop(
                journal::prepare_resizable_v3_expected(
                    &mut target,
                    &mut log,
                    &source.before,
                    &source.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
        }
        let writes = log.writes;
        if version < 3 {
            // This nominal native qualifier accepts only CheckedV3, never an
            // unchecked stream or a Checked V1/V2 owner.
            assert!(journal::inspect_v3(&mut log, &mut budget(), &|| false).is_err());
        } else {
            let checked = journal::inspect_v3(&mut log, &mut budget(), &|| false).unwrap();
            let original = Intent::decode(&wire(&source), &mut budget(), &|| false).unwrap();
            assert!(matches!(
                original.qualify_retained_recovery(
                    checked,
                    CatalogScope::new(31815),
                    &mut budget(),
                    &|| true
                ),
                Err(Error::Inspect(crate::InspectError::Cancelled))
            ));
            let checked = journal::inspect_v3(&mut log, &mut budget(), &|| false).unwrap();
            let original = Intent::decode(&wire(&source), &mut budget(), &|| false).unwrap();
            assert!(
                original
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(31816),
                        &mut tablecraft_core::limits::ResourceBudget::new(0, 0, 0),
                        &|| false
                    )
                    .is_err()
            );
        }
        assert_eq!(target.writes, 0);
        assert_eq!(log.writes, writes);
        same(target.bytes.get_ref(), &source.before);
    }
}

#[test]
fn sealed_create_effect_policy_cannot_cross_old_or_unique_original_capture() {
    let source = source(false);
    let unique = image(&source.before)
        .prepare_retained_unique_long_index_creation(
            source.table,
            &UniqueLongIndexCreate::new(source.index_name, source.column, false),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(
        crate::transaction::intent::fresh_index_create::NativeFreshIndexCreateIntent::capture(
            unique.into_parts(),
            &mut budget(),
            &|| false,
        ),
        Err(Error::InvalidRecord)
    ));
    let ordinary = image(&source.before)
        .prepare_retained_fresh_long_index_creation(
            source.table,
            &crate::SecondaryIndexCreate::new(source.index_name, source.column, false),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(
        Intent::capture(ordinary.into_parts(), &mut budget(), &|| false),
        Err(Error::InvalidRecord)
    ));
}
