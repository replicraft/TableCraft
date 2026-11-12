//! Fresh source reconstruction binds DATA and complete INDEX effects together.
use super::{Cell, budget, groups, literal};
use crate::{NativeImage, NativeUpdateIntent, RecoverableNativeEditPlan};
use std::io::Cursor;
use tablecraft_io::journal::{self, RecoveryOutcome, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::secondary_index::normal::{Population, updating as raw},
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
fn cells(case: &raw::Case) -> Vec<Cell> {
    case.cells
        .iter()
        .map(|cell| Cell {
            row: cell.row,
            expected: cell.expected,
            value: cell.value,
        })
        .collect()
}
fn wire(case: &raw::Case) -> Vec<u8> {
    literal(
        &case.creation.expected,
        case.creation.table,
        case.creation.column,
        &cells(case),
    )
}
fn decode(case: &raw::Case) -> NativeUpdateIntent {
    NativeUpdateIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}
fn plan(case: &raw::Case) -> RecoverableNativeEditPlan {
    image(&case.creation.expected, 32620)
        .plan_recoverable_retained_indexed_long_updates(
            case.creation.table,
            groups(case.creation.column, &cells(case)),
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
fn indexed_original_rebuild_and_v3_recovery_keep_data_and_tree_atomic() {
    for descending in [false, true] {
        for case in [
            raw::source(Population::NullableDuplicate, descending),
            raw::protected_companion(descending),
        ] {
            let captured = plan(&case);
            assert_eq!(
                captured.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case)
            );
            same(captured.after_bytes(), &case.expected);
            let rebuilt = captured
                .requalify_with_budgets(
                    image(&case.creation.expected, 32621),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            same(rebuilt.after_bytes(), &case.expected);
            for state in 0..3 {
                let mut target = MemoryStorage::new(case.creation.expected.to_vec());
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
                        // A genuine mixed image retains new DATA while restoring
                        // the old selected INDEX page. Recovery owns both parts.
                        let root = case.creation.root as usize * 4096;
                        target.bytes.get_mut()[root..root + 4096]
                            .copy_from_slice(&case.creation.expected[root..root + 4096]);
                    }
                }
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                let qualified = decode(&case)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(32622),
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
                        &case.creation.expected
                    },
                );
                let writes = target.writes;
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                decode(&case)
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(32623),
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

fn reject(case: &raw::Case, wire: &[u8], after: &[u8]) {
    let mut target = MemoryStorage::new(case.creation.expected.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &case.creation.expected,
            after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let writes = journal_file.writes;
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(
        NativeUpdateIntent::decode(wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(32624), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, writes));
    same(target.bytes.get_ref(), &case.creation.expected);
}

#[test]
fn forged_tree_foreign_companion_late_expected_and_stale_source_have_no_effects() {
    let case = raw::protected_companion(false);
    let mut wrong = wire(&case);
    wrong[24] ^= 1;
    reject(&case, &wrong, &case.expected);
    let mut wrong_cells = cells(&case);
    wrong_cells[2].expected = Some(999);
    reject(
        &case,
        &literal(
            &case.creation.expected,
            case.creation.table,
            case.creation.column,
            &wrong_cells,
        ),
        &case.expected,
    );
    for offset in [
        case.creation.root as usize * 4096 + 490,
        case.creation.target_definition as usize * 4096 + 3000,
        3 * 4096 + 3000,
    ] {
        let mut after = case.expected.to_vec();
        after[offset] ^= 1;
        reject(&case, &wire(&case), &after);
    }
    let mut old = wire(&case);
    old[..8].copy_from_slice(b"TCNI0017");
    old[12..14].copy_from_slice(&17_u16.to_le_bytes());
    reject(&case, &old, &case.expected);
    let mut stale = case.creation.expected.to_vec();
    stale[3 * 4096 + 3000] ^= 1;
    let mut target = MemoryStorage::new(stale);
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        plan(&case)
            .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!((target.writes, journal_file.writes), (0, 0));
    assert!(
        plan(&case)
            .requalify_with_budgets(
                image(&case.creation.expected, 32625),
                &mut budget(),
                &mut budget(),
                &|| true
            )
            .is_err()
    );
}

#[test]
fn indexed_original_has_no_v1_or_v2_recovery_authority() {
    use crate::{
        NativeIntentError, NativeRecoveryQualificationError, qualify_native_allocation_recovery,
        qualify_native_update_recovery,
    };
    let case = raw::source(Population::NullableDuplicate, false);
    for version in [1, 2] {
        let mut target = MemoryStorage::new(case.creation.expected.to_vec());
        let mut journal_file = MemoryStorage::new(Vec::new());
        if version == 1 {
            drop(
                journal::prepare_expected(
                    &mut target,
                    &mut journal_file,
                    &case.creation.expected,
                    &case.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let checked = journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).unwrap();
            let refused = qualify_native_update_recovery(
                checked,
                decode(&case),
                CatalogScope::new(32626),
                &mut budget(),
                &|| false,
            );
            assert!(matches!(
                refused,
                Err(NativeRecoveryQualificationError::Intent(
                    NativeIntentError::UnsupportedProfile
                ))
            ));
        } else {
            drop(
                journal::prepare_resizable_expected(
                    &mut target,
                    &mut journal_file,
                    &case.creation.expected,
                    &case.expected,
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            );
            let checked = journal::inspect_v2(&mut journal_file, &mut budget(), &|| false).unwrap();
            let refused = qualify_native_allocation_recovery(
                checked,
                decode(&case),
                CatalogScope::new(32627),
                &mut budget(),
                &|| false,
            );
            assert!(matches!(
                refused,
                Err(NativeRecoveryQualificationError::Intent(
                    NativeIntentError::UnsupportedProfile
                ))
            ));
        }
        assert_eq!(target.writes, 0);
        same(target.bytes.get_ref(), &case.creation.expected);
    }
}

#[test]
fn fresh_recovery_cannot_turn_preexisting_unique_or_required_violations_into_effects() {
    for required in [false, true] {
        let case = raw::unique(false, required);
        let mut wrong = cells(&case);
        wrong[0].value = Some(2);
        reject(
            &case,
            &literal(
                &case.creation.expected,
                case.creation.table,
                case.creation.column,
                &wrong,
            ),
            &case.expected,
        );
        if required {
            wrong[0].value = None;
            reject(
                &case,
                &literal(
                    &case.creation.expected,
                    case.creation.table,
                    case.creation.column,
                    &wrong,
                ),
                &case.expected,
            );
        }
    }
}
