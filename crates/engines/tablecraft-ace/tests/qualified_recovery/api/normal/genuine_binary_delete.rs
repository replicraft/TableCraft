//! Source-bound Binary DELETE, original TCND requests and qualified recovery.
#[path = "genuine_binary_delete/fixtures.rs"]
mod fixtures;
#[path = "genuine_binary_delete/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use fixtures::Case;
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, NativeDeleteIntent, NativeImage, NativeIntentError,
    NativeRecoveryQualificationError, RecoverableEditError, RowDelete, check_table,
    inspect_table_index_counter, qualify_native_delete_recovery, read_table, read_table_index,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_binary_delete as sources;

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(case: &Case, reverse: bool) -> Vec<RowDelete> {
    fixtures::witnesses(case, reverse)
        .into_iter()
        .map(|(row, bytes)| RowDelete::new(row, bytes))
        .collect()
}

fn wire(case: &Case, reverse: bool) -> Vec<u8> {
    let selected = fixtures::witnesses(case, reverse);
    let mut body = Vec::new();
    for (ordinal, raw) in &selected {
        body.extend_from_slice(&(*ordinal as u64).to_le_bytes());
        body.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        body.extend_from_slice(raw);
    }
    let total = 68 + "BinaryKeys".len() + body.len();
    let mut result = Vec::from(*b"TCND0001");
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&[1, 0, 0, 0]);
    result.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    result.extend_from_slice(&digest.finalize());
    for word in ["BinaryKeys".len() as u32, selected.len() as u32, 0] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.extend_from_slice(b"BinaryKeys");
    result.extend_from_slice(&body);
    assert_eq!(result.len(), total);
    result
}

fn intent(case: &Case) -> NativeDeleteIntent {
    NativeDeleteIntent::decode(&wire(case, false), &mut budget(), &|| false).unwrap()
}

fn conflict(error: RecoverableEditError) {
    assert!(matches!(
        error,
        RecoverableEditError::Edit(EditError::Conflict)
            | RecoverableEditError::Intent(NativeIntentError::Request(EditError::Conflict)),
    ));
}

#[test]
#[ignore = "requires fourteen native/SDK-qualified Binary DELETE source views"]
fn binary_delete_literal_commits_retain_original_tcnd_orders_aliases_history_and_empty_tables() {
    let literals = fixtures::cases()
        .into_iter()
        .map(|case| {
            let forward = wire(&case, false);
            let reverse = wire(&case, true);
            (case, forward, reverse)
        })
        .collect::<Vec<_>>();
    let mut committed = 0;
    for (case, forward_wire, reverse_wire) in literals {
        let orders: &[bool] = if case.recipe.selected().len() > 1 {
            &[false, true]
        } else {
            &[false]
        };
        for &reverse in orders {
            let prepared = image(&case.before)
                .prepare_ordinary_row_deletes(
                    "BinaryKeys",
                    case.recipe.selected(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(prepared.len(), case.recipe.selected().len());
            for (actual, (row, bytes)) in prepared.iter().zip(fixtures::witnesses(&case, false)) {
                assert_eq!(actual.row(), row);
                assert_eq!(actual.expected_bytes(), bytes);
            }
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "BinaryKeys",
                    requests(&case, reverse),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(plan.deleted_rows(), case.recipe.selected().len());
            let expected_wire = if reverse {
                &reverse_wire
            } else {
                &forward_wire
            };
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                *expected_wire
            );
            assert_eq!(
                NativeDeleteIntent::decode(expected_wire, &mut budget(), &|| false)
                    .unwrap()
                    .encode(&mut budget(), &|| false)
                    .unwrap(),
                *expected_wire,
            );
            for (actual, (row, bytes)) in plan
                .intent()
                .deletes()
                .iter()
                .zip(fixtures::witnesses(&case, reverse))
            {
                assert_eq!(actual.row(), row);
                assert_eq!(actual.expected_bytes(), bytes);
            }
            let mut target = Storage::new(case.before.clone());
            let mut log = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut log, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                case.recipe.selected().len(),
            );
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                log.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed)
            );
            assert_eq!(
                read_table(
                    &mut Cursor::new(&case.after),
                    scope(),
                    "BinaryKeys",
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .rows(),
                sources::final_rows(case.recipe)
            );
            let checked = check_table(
                &mut Cursor::new(&case.after),
                scope(),
                "BinaryKeys",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(checked.rows(), 13 - case.recipe.selected().len());
            assert_eq!(checked.indexes().len(), 6);
            assert!(
                checked
                    .indexes()
                    .iter()
                    .all(|index| index.status() == IndexCheckStatus::Checked)
            );
            for (left, right, history) in [
                ("PrimaryID", "PrimaryAlias", 13),
                ("Ascending", "AscendingAlias", 12),
                ("Descending", "DescendingAlias", 12),
            ] {
                let first = read_table_index(
                    &mut Cursor::new(&case.after),
                    scope(),
                    "BinaryKeys",
                    left,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let alias = read_table_index(
                    &mut Cursor::new(&case.after),
                    scope(),
                    "BinaryKeys",
                    right,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(first.rows(), alias.rows());
                assert_eq!(first.rows().len(), checked.rows());
                let actual = inspect_table_index_counter(
                    &mut Cursor::new(&case.after),
                    scope(),
                    "BinaryKeys",
                    left,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(actual.value(), history);
            }
            committed += 1;
        }
        for before in [&case.before, &case.after] {
            let empty = image(before)
                .plan_recoverable_ordinary_row_deletes("BinaryKeys", vec![], &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(empty.deleted_rows(), 0);
            assert_eq!(empty.before_bytes(), before);
            assert_eq!(empty.after_bytes(), before);
        }
    }
    assert_eq!(committed, 18);
}

#[test]
#[ignore = "requires fourteen native/SDK-qualified Binary DELETE source views"]
fn binary_delete_stale_masked_wrong_ordinal_and_duplicate_witnesses_refuse_whole_plans() {
    for case in fixtures::cases() {
        let first = case.recipe.selected()[0];
        let mut stale = fixtures::record(first);
        *stale.last_mut().unwrap() ^= 2;
        conflict(
            image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "BinaryKeys",
                    vec![RowDelete::new(first, stale)],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap(),
        );
        let other = (first + 1) % 13;
        conflict(
            image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "BinaryKeys",
                    vec![RowDelete::new(other, fixtures::record(first))],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap(),
        );
        conflict(
            image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    "BinaryKeys",
                    vec![
                        RowDelete::new(first, fixtures::record(first)),
                        RowDelete::new(first, fixtures::record(first)),
                    ],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap(),
        );
    }
}

#[test]
#[ignore = "requires fourteen native/SDK-qualified Binary DELETE source views"]
fn binary_tcnd_recovery_uses_same_checked_journal_for_true_mixed_and_committed_mismatch() {
    let mut contexts = 0;
    for case in fixtures::cases() {
        let td = case.definition as usize * 4096;
        let mut mixed = case.before.clone();
        mixed[td..td + 4096].copy_from_slice(&case.after[td..td + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            for initial in [case.after.clone(), mixed.clone()] {
                let refuse = matches!(marker, Marker::Committed) && initial == mixed;
                let outcome = if matches!(marker, Marker::Committed) {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                };
                let expected = if matches!(marker, Marker::Committed) {
                    &case.after
                } else {
                    &case.before
                };
                let mut target = Storage::new(initial.clone());
                let mut log = Storage::new(journals::journal(&case.before, &case.after, marker));
                let saved = log.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_delete_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let writes = target.writes;
                    let result = qualified.recover(&mut target, &mut budget(), &|| false);
                    if refuse {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.bytes.get_ref(), &initial);
                        assert_eq!(target.writes, writes);
                    } else {
                        assert_eq!(result.unwrap(), outcome);
                        assert_eq!(target.bytes.get_ref(), expected);
                    }
                }
                let writes = target.writes;
                let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_delete_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let result = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if refuse {
                    assert!(matches!(result, Err(JournalError::TargetMismatch)));
                } else {
                    assert_eq!(result.unwrap(), outcome);
                }
                assert_eq!(target.writes, writes);
                assert_eq!(log.writes, 0);
                assert_eq!(log.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 72);
}

#[test]
#[ignore = "requires fourteen native/SDK-qualified Binary DELETE source views"]
fn binary_tcnd_qualification_rejects_mask_data_opaque_tail_history_and_map_forgeries() {
    let mut refused = 0;
    for case in fixtures::cases() {
        for at in fixtures::forged_offsets(&case) {
            let mut after = case.after.clone();
            after[at] ^= 1;
            let mut log = Storage::new(journals::journal(&case.before, &after, Marker::Pending));
            let saved = log.bytes.get_ref().clone();
            let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_delete_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(log.writes, 0);
            assert_eq!(log.bytes.get_ref(), &saved);
            refused += 1;
        }
    }
    assert_eq!(refused, 120);
}
