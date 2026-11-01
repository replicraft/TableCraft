//! Source-qualified exact numeric carriers, original intent wires and durable replay.
#[path = "sparse_numeric/admission.rs"]
mod admission;
#[path = "sparse_numeric/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, FixedScalarUpdate, NativeDeleteIntent, NativeFixedIntentGroup, NativeImage,
    NativeInsertIntent, NativeRecoveryQualificationError, RecoverableEditError, RowDelete,
    RowInsert, WriteBlocker, qualify_native_delete_recovery, qualify_native_insert_recovery,
    read_table,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::qualified_sparse_numeric as sources;

fn cases() -> Vec<sources::Case> {
    sources::load_all(|raw, expected| {
        let hash = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(hash, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn inserts(case: &sources::Case) -> Vec<RowInsert> {
    vec![RowInsert::new(case.request())]
}

fn deletes(case: &sources::Case, reverse: bool) -> Vec<RowDelete> {
    if reverse {
        vec![2, 1, 0]
    } else {
        vec![0, 1, 2]
    }
    .into_iter()
    .map(|slot| RowDelete::new(slot, case.record(slot)))
    .collect()
}

fn wire(case: &sources::Case, reverse: bool) -> Vec<u8> {
    let mut body = Vec::new();
    let count;
    if case.operation == "insert" {
        let rows = vec![case.request()];
        count = rows.len();
        for values in rows {
            let mut payload = Vec::from(3_u32.to_le_bytes());
            for value in values {
                let (tag, bytes) = match value {
                    Value::Null => (0, Vec::new()),
                    Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
                    Value::Int64(value) => (3, value.to_le_bytes().to_vec()),
                    Value::Currency(value) => (4, value.scaled().to_le_bytes().to_vec()),
                    _ => panic!("outside original request carrier profile"),
                };
                payload.extend_from_slice(&[tag, 0, 0, 0]);
                payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                payload.extend_from_slice(&bytes);
            }
            body.extend_from_slice(&((4 + payload.len()) as u32).to_le_bytes());
            body.extend_from_slice(&payload);
        }
    } else {
        let selected = if reverse {
            vec![2, 1, 0]
        } else {
            vec![0, 1, 2]
        };
        count = selected.len();
        for slot in selected {
            body.extend_from_slice(&(slot as u64).to_le_bytes());
            body.extend_from_slice(&(case.record_bytes() as u32).to_le_bytes());
            body.extend_from_slice(&case.record(slot));
        }
    }
    let total = 68 + case.table.len() + body.len();
    let mut raw = if case.operation == "insert" {
        Vec::from(*b"TCNA0001")
    } else {
        Vec::from(*b"TCND0001")
    };
    raw.extend_from_slice(&(total as u32).to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    raw.extend_from_slice(&digest.finalize());
    for value in [case.table.len() as u32, count as u32, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(case.table.as_bytes());
    raw.extend_from_slice(&body);
    assert_eq!(raw.len(), total);
    raw
}

#[test]
#[ignore = "requires120 generic native source contexts and128 crossing-byte SDK reads"]
fn sparse_actions_commit_literal_images_and_original_wires_in_both_delete_orders() {
    let mut commits = 0;
    for case in cases() {
        let orders: &[bool] = if case.operation == "delete" && case.recipe == "delete-all" {
            &[false, true]
        } else {
            &[false]
        };
        for &reverse in orders {
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            if case.operation == "insert" {
                let expected = vec![case.request()];
                let plan = image(&case.before)
                    .plan_recoverable_ordinary_row_inserts(
                        case.table,
                        inserts(&case),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.inserted_rows(), expected.len());
                for (actual, original) in plan.intent().rows().iter().zip(expected) {
                    assert_eq!(actual.values(), original);
                }
                assert_eq!(
                    plan.intent().encode(&mut budget(), &|| false).unwrap(),
                    wire(&case, false)
                );
                assert_eq!(
                    plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap()
                        .commit(&mut budget(), &|| false)
                        .unwrap(),
                    plan_count(&case)
                );
            } else {
                let selected = if reverse {
                    vec![2, 1, 0]
                } else {
                    vec![0, 1, 2]
                };
                let prepared = image(&case.before)
                    .prepare_ordinary_row_deletes(case.table, &selected, &mut budget(), &|| false)
                    .unwrap();
                for actual in prepared {
                    assert_eq!(actual.expected_bytes(), case.record(actual.row()));
                }
                let plan = image(&case.before)
                    .plan_recoverable_ordinary_row_deletes(
                        case.table,
                        deletes(&case, reverse),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.after_bytes(), case.after);
                assert_eq!(plan.deleted_rows(), selected.len());
                assert_eq!(
                    plan.intent().encode(&mut budget(), &|| false).unwrap(),
                    wire(&case, reverse)
                );
                assert_eq!(
                    plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap()
                        .commit(&mut budget(), &|| false)
                        .unwrap(),
                    selected.len()
                );
            }
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, &case.after, Marker::Committed)
            );
            let rows = read_table(
                &mut Cursor::new(target.bytes.get_ref()),
                scope(),
                case.table,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(rows.rows(), case.rows());
            commits += 1;
        }
    }
    assert_eq!(commits, 48);
}

fn plan_count(case: &sources::Case) -> usize {
    if case.operation == "insert" { 1 } else { 3 }
}

#[test]
#[ignore = "requires source qualification before action-framer widening"]
fn action_admission_does_not_widen_legacy_canonical_source_or_updates() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-zero")
    {
        let original = image(&case.before);
        let snapshot = read_table(
            &mut original.reader(),
            scope(),
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(snapshot.rows(), case.original_rows());
        for (position, field) in snapshot.schema().columns().iter().enumerate() {
            assert_eq!(field.number(), case.numbers()[position]);
            let at = case.definition as usize * 4096 + 99 + 25 * position + 9;
            assert_eq!(
                u16::from_le_bytes(case.before[at..at + 2].try_into().unwrap()),
                case.stored_ids()[position]
            );
        }
        let at = case.definition as usize * 4096 + 41;
        assert_eq!(
            u16::from_le_bytes(case.before[at..at + 2].try_into().unwrap()),
            case.maximum()
        );
        assert_eq!(
            original
                .check_native_ordinary_fixed_table(case.table, &mut budget(), &|| false)
                .err()
                .unwrap(),
            EditError::Unsupported(WriteBlocker::ActiveContent)
        );
        let group = NativeFixedIntentGroup::new(
            "Marker",
            vec![FixedScalarUpdate::new(
                0,
                Value::Int32(17),
                Value::Int32(42),
            )],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let error = image(&case.before)
            .plan_recoverable_ordinary_fixed_updates(
                case.table,
                vec![group],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::ActiveContent))
        ));
        let prepared = image(&case.before)
            .prepare_ordinary_row_deletes(case.table, &[0, 1, 2], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(prepared.len(), 3);
        for row in prepared {
            assert_eq!(row.expected_bytes(), case.record(row.row()));
        }
        let insert = image(&case.before)
            .plan_recoverable_ordinary_row_inserts(case.table, Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(insert.inserted_rows(), 0);
        assert_eq!(insert.after_bytes(), case.before);
        let delete = image(&case.before)
            .plan_recoverable_ordinary_row_deletes(case.table, Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(delete.deleted_rows(), 0);
        assert_eq!(delete.after_bytes(), case.before);
    }
}

#[test]
#[ignore = "requires source-qualified literal sparse action images"]
fn sparse_action_recovery_requalifies_original_numbers_and_recovers_causal_index_mixtures() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-zero")
    {
        let raw = wire(&case, false);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let mut initial = case.after.clone();
            if matches!(marker, Marker::Partial) {
                initial = case.before.clone();
                let at = case.roots[1] as usize * 4096;
                initial[at..at + 4096].copy_from_slice(&case.after[at..at + 4096]);
                assert_ne!(initial, case.before);
                assert_ne!(initial, case.after);
            }
            let mut target = Storage::new(initial);
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let committed = matches!(marker, Marker::Committed);
            let expected = if committed { &case.after } else { &case.before };
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent = NativeInsertIntent::decode(&raw, &mut budget(), &|| false).unwrap();
                let actual = qualify_native_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .recover(&mut target, &mut budget(), &|| false)
                .unwrap();
                assert_eq!(
                    actual,
                    if committed {
                        RecoveryOutcome::Committed
                    } else {
                        RecoveryOutcome::RolledBack
                    }
                );
                assert_eq!(target.bytes.get_ref(), expected);
            }
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent = NativeInsertIntent::decode(&raw, &mut budget(), &|| false).unwrap();
            qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false)
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
    for case in cases()
        .into_iter()
        .filter(|case| case.operation == "delete")
    {
        for marker in [Marker::Pending, Marker::Committed] {
            let mut target = Storage::new(case.after.clone());
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent =
                NativeDeleteIntent::decode(&wire(&case, true), &mut budget(), &|| false).unwrap();
            let actual =
                qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .unwrap()
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap();
            let committed = matches!(marker, Marker::Committed);
            assert_eq!(
                actual,
                if committed {
                    RecoveryOutcome::Committed
                } else {
                    RecoveryOutcome::RolledBack
                }
            );
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &case.after } else { &case.before }
            );
        }
    }
}

#[test]
#[ignore = "requires source-qualified literal sparse action images"]
fn sparse_journal_after_masks_inactive_payload_ids_and_original_requests_are_bound_before_effects()
{
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-zero")
    {
        let page = case.data as usize * 4096;
        let length = case.record_bytes();
        let original = wire(&case, false);
        for at in [
            page + 4096 - 4 * length + 18,
            page + 4096 - 2 * length + 6,
            case.definition as usize * 4096 + 99 + 25 + 9,
            case.roots[1] as usize * 4096 + 480,
        ] {
            let mut forged = case.after.clone();
            forged[at] ^= 2;
            let journal_bytes = journals::journal(&case.before, &forged, Marker::Pending);
            let target = Storage::new(case.after.clone());
            let mut journal = Storage::new(journal_bytes.clone());
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent = NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(target.bytes.get_ref(), &case.after);
            assert_eq!(target.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &journal_bytes);
            assert_eq!(journal.writes, 0);
        }
        let mut altered = original.clone();
        *altered.last_mut().unwrap() ^= 1;
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            Marker::Committed,
        ));
        let retained = journal.bytes.get_ref().clone();
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        let intent = NativeInsertIntent::decode(&altered, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(journal.bytes.get_ref(), &retained);
        assert_eq!(journal.writes, 0);
    }
}
