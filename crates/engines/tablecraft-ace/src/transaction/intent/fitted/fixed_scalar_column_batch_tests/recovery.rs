//! Cold fixed-scalar recovery rederives the whole effect from immutable original requests.
use super::{budget, groups, literal};
use crate::{
    NativeImage, NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent,
    RecoverableNativeEditPlan, qualify_native_allocation_recovery, qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::fixed_scalars::{
        COLUMNS_TO_EDIT, Column, batch as raw,
    },
    support::io::journal::MemoryStorage,
};

fn plan(case: &raw::Source) -> RecoverableNativeEditPlan {
    NativeImage::capture_retained(
        &mut Cursor::new(&case.before),
        CatalogScope::new(4327),
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .plan_recoverable_retained_fixed_scalar_column_batch_updates(
        case.table,
        groups(case.column, &case.cells),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn decode(case: &raw::Source) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(
        &literal(&case.before, case.column, &case.cells),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn pending_mixed_committed_and_settled_fixed_scalar_journals_keep_the_same_atomic_identity() {
    for column in COLUMNS_TO_EDIT {
        for case in [
            raw::values(
                column,
                &[Some(raw::initial(column)), None],
                &[None, Some(raw::replacement(column))],
            ),
            raw::values(
                column,
                &[Some(raw::initial(column)), None],
                &[Some(raw::initial(column)), None],
            ),
            raw::empty(column),
            raw::forwarded(column),
            raw::materializing(column),
        ] {
            assert_eq!(
                plan(&case)
                    .intent()
                    .encode(&mut budget(), &|| false)
                    .unwrap(),
                literal(&case.before, case.column, &case.cells)
            );
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
                        let changed = case
                            .before
                            .iter()
                            .zip(&case.expected)
                            .enumerate()
                            .filter_map(|(offset, (before, after))| {
                                (before != after).then_some(offset)
                            })
                            .collect::<Vec<_>>();
                        if changed.is_empty() {
                            assert_eq!(case.before, case.expected);
                        } else {
                            assert!(changed.len() >= 2);
                            let split = changed[changed.len() / 2];
                            target.bytes.get_mut()[..split]
                                .copy_from_slice(&case.expected[..split]);
                            assert!(target.bytes.get_ref() != &case.before);
                            assert!(target.bytes.get_ref() != &case.expected);
                        }
                    }
                }
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                let qualified = decode(&case)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(4328),
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
                assert_eq!(
                    target.bytes.get_ref(),
                    if state == 2 {
                        &case.expected
                    } else {
                        &case.before
                    }
                );
                let writes = target.writes;
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                decode(&case)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(4329),
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
}

fn refuse(case: &raw::Source, original: NativeUpdateIntent, after: &[u8]) {
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
            .qualify_retained_recovery(checked, CatalogScope::new(4330), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, writes));
    assert_eq!(target.bytes.get_ref(), &case.before);
}

#[test]
fn stale_source_late_expected_tampered_effect_and_older_protocols_grant_no_writes() {
    let column = Column::LargeValue;
    let case = raw::values(
        column,
        &[Some(raw::initial(column)), None],
        &[None, Some(raw::replacement(column))],
    );
    let mut stale = literal(&case.before, case.column, &case.cells);
    stale[24] ^= 1;
    refuse(
        &case,
        NativeUpdateIntent::decode(&stale, &mut budget(), &|| false).unwrap(),
        &case.expected,
    );
    let mut wrong = case.cells.to_vec();
    wrong[1].expected = Some(raw::initial(column));
    refuse(
        &case,
        NativeUpdateIntent::decode(
            &literal(&case.before, case.column, &wrong),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
        &case.expected,
    );
    let mut altered = case.expected.to_vec();
    altered[3 * 4096 + 3000] ^= 1;
    refuse(&case, decode(&case), &altered);
    let mut old21 = literal(&case.before, case.column, &case.cells);
    old21[..8].copy_from_slice(b"TCNI0021");
    old21[12..14].copy_from_slice(&21_u16.to_le_bytes());
    old21[72 + case.table.len() + 13] = 0;
    // Old21 data carries these scalar cells only after clearing the new descriptor.
    // Its consuming native singleton remains closed for this batch.
    refuse(
        &case,
        NativeUpdateIntent::decode(&old21, &mut budget(), &|| false).unwrap(),
        &case.expected,
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
                    CatalogScope::new(4331),
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
            let checked = journal::inspect_v2(&mut journal_file, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_allocation_recovery(
                    checked,
                    decode(&case),
                    CatalogScope::new(4332),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::Intent(
                    NativeIntentError::UnsupportedProfile
                ))
            ));
        }
        assert_eq!(target.writes, 0);
        assert_eq!(target.bytes.get_ref(), &case.before);
    }
}
