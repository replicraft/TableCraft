//! Literal Text cardinality commits and original-intent V1 checked recovery.
#[path = "text_cardinality/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, resizable_storage::Target, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, NativeCompleteInsertIntent,
    NativeDeleteIntent, NativeImage, NativeIntentError, NativeRecoveryQualificationError,
    RecoverableEditError, RowDelete, RowInsert, WriteBlocker, inspect_table_index_counter,
    qualify_native_complete_insert_recovery, qualify_native_delete_recovery, read_table,
    read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_image, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_text_cardinality::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn inserts(case: &Case) -> Vec<RowInsert> {
    case.insert_values()
        .into_iter()
        .map(RowInsert::new)
        .collect()
}

fn deletes(case: &Case) -> Vec<RowDelete> {
    case.witnesses()
        .into_iter()
        .map(|(ordinal, raw)| RowDelete::new(ordinal, raw))
        .collect()
}

fn wire(case: &Case) -> Vec<u8> {
    let mut body = Vec::new();
    if case.action == "insert" {
        for row in case.insert_values() {
            let mut payload = Vec::from(2_u32.to_le_bytes());
            for value in row {
                let (tag, raw) = match value {
                    Value::Null => (0, Vec::new()),
                    Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
                    Value::Text(value) => (6, value.into_bytes()),
                    _ => panic!("closed source carrier"),
                };
                payload.extend_from_slice(&[tag, 0, 0, 0]);
                payload.extend_from_slice(&(raw.len() as u32).to_le_bytes());
                payload.extend_from_slice(&raw);
            }
            body.extend_from_slice(&((payload.len() + 4) as u32).to_le_bytes());
            body.extend_from_slice(&payload);
        }
    } else {
        for (ordinal, raw) in case.witnesses() {
            body.extend_from_slice(&(ordinal as u64).to_le_bytes());
            body.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            body.extend_from_slice(&raw);
        }
    }
    let total = 68 + case.table.len() + body.len();
    let mut result = Vec::from(if case.action == "insert" {
        *b"TCNA0004"
    } else {
        *b"TCND0001"
    });
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&[if case.action == "insert" { 4 } else { 1 }, 0, 0, 0]);
    result.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    result.extend_from_slice(&digest.finalize());
    for word in [case.table.len() as u32, case.changed_rows() as u32, 0] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.extend_from_slice(case.table.as_bytes());
    result.extend_from_slice(&body);
    assert_eq!(result.len(), total);
    result
}

fn settle(
    case: &Case,
    target: &mut Target,
    log: &mut Storage,
    confirm: bool,
) -> Result<RecoveryOutcome, JournalError> {
    if case.action == "insert" {
        let intent =
            NativeCompleteInsertIntent::decode(&wire(case), &mut budget(), &|| false).unwrap();
        let checked = inspect_image(log, &mut budget(), &|| false).unwrap();
        let qualified = qualify_native_complete_insert_recovery(
            checked,
            intent,
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        if confirm {
            qualified.confirm_completed(target, &mut budget(), &|| false)
        } else {
            qualified.recover(target, &mut budget(), &|| false)
        }
    } else {
        let intent = NativeDeleteIntent::decode(&wire(case), &mut budget(), &|| false).unwrap();
        let checked = inspect_v1(log, &mut budget(), &|| false).unwrap();
        let qualified =
            qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false)
                .unwrap();
        if confirm {
            qualified.confirm_completed(target, &mut budget(), &|| false)
        } else {
            qualified.recover(target, &mut budget(), &|| false)
        }
    }
}

#[test]
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn text_cardinality_commits_retain_original_intents_rows_aliases_and_histories() {
    let literals = cases()
        .into_iter()
        .map(|case| {
            let expected = wire(&case);
            (case, expected)
        })
        .collect::<Vec<_>>();
    let mut counts = [0; 2];
    let mut coverage_denials = 0;
    for (case, expected_wire) in literals {
        if case.recipe == "insert-empty" {
            let error = image(&case.before)
                .plan_recoverable_ordinary_complete_row_inserts(
                    case.table,
                    inserts(&case),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::ConstraintCoverage
                ))
            ));
            coverage_denials += 1;
            continue;
        }
        let mut target = Target::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        if case.action == "insert" {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_complete_row_inserts(
                    case.table,
                    inserts(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                expected_wire
            );
            assert_eq!(plan.inserted_rows(), case.changed_rows());
            assert_eq!(
                plan.prepare_insert(&mut target, &mut log, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                case.changed_rows()
            );
            counts[0] += 1;
        } else {
            let prepared = image(&case.before)
                .prepare_ordinary_row_deletes(
                    case.table,
                    &case.deleted_slots(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut expected_witnesses = case.witnesses();
            expected_witnesses.sort_by_key(|(ordinal, _)| *ordinal);
            assert_eq!(prepared.len(), expected_witnesses.len());
            for (actual, (ordinal, raw)) in prepared.iter().zip(expected_witnesses) {
                assert_eq!(actual.row(), ordinal);
                assert_eq!(actual.expected_bytes(), raw);
            }
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    case.table,
                    deletes(&case),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), case.after);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                expected_wire
            );
            assert_eq!(
                plan.prepare(&mut target, &mut log, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                case.changed_rows()
            );
            counts[1] += 1;
        }
        assert_eq!(target.storage.bytes.get_ref(), &case.after);
        assert_eq!(target.resizes, 0);
        let observed = read_table(
            &mut Cursor::new(&case.after),
            scope(),
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(observed.rows(), case.final_rows());
        let asc = read_table_index(
            &mut Cursor::new(&case.after),
            scope(),
            case.table,
            "Asc",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let twin = read_table_index(
            &mut Cursor::new(&case.after),
            scope(),
            case.table,
            "Twin",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(asc.rows(), twin.rows());
        assert_eq!(asc.rows().len(), case.final_rows().len());
        for (name, expected) in [
            (
                "Primary",
                if case.action == "insert" {
                    24 + case.changed_rows()
                } else {
                    24
                },
            ),
            ("Asc", if case.table == "TextLegacyLeaf" { 7 } else { 8 }),
            ("Desc", if case.table == "TextLegacyLeaf" { 7 } else { 8 }),
        ] {
            let counter = inspect_table_index_counter(
                &mut Cursor::new(&case.after),
                scope(),
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(counter.value() as usize, expected);
        }
        let writes = (target.storage.writes, target.resizes, log.writes);
        assert_eq!(
            settle(&case, &mut target, &mut log, true).unwrap(),
            RecoveryOutcome::Committed
        );
        assert_eq!((target.storage.writes, target.resizes, log.writes), writes);
    }
    assert_eq!(counts, [24, 36]);
    assert_eq!(coverage_denials, 4);
}

#[test]
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn text_cardinality_checked_recovery_handles_mixed_pages_and_protected_forgeries() {
    let mut contexts = 0;
    let mut forgeries = 0;
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.recipe, "batch-a-null" | "delete-case-pair"))
    {
        let mut mixed = case.before.clone();
        let base = case.data as usize * 4096;
        mixed[base..base + 4096].copy_from_slice(&case.after[base..base + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            for initial in [case.after.clone(), mixed.clone()] {
                let refuse = matches!(marker, Marker::Committed) && initial == mixed;
                let mut target = Target::new(initial.clone());
                let saved = journals::journal(&case.before, &case.after, marker);
                let mut log = Storage::new(saved.clone());
                for _ in 0..2 {
                    let writes = (target.storage.writes, target.resizes, log.writes);
                    let result = settle(&case, &mut target, &mut log, false);
                    if refuse {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!((target.storage.writes, target.resizes, log.writes), writes);
                        assert_eq!(target.storage.bytes.get_ref(), &initial);
                    } else {
                        let committed = matches!(marker, Marker::Committed);
                        assert_eq!(
                            result.unwrap(),
                            if committed {
                                RecoveryOutcome::Committed
                            } else {
                                RecoveryOutcome::RolledBack
                            }
                        );
                        assert_eq!(
                            target.storage.bytes.get_ref(),
                            if committed { &case.after } else { &case.before }
                        );
                    }
                }
                let writes = (target.storage.writes, target.resizes, log.writes);
                let result = settle(&case, &mut target, &mut log, true);
                if refuse {
                    assert!(matches!(result, Err(JournalError::TargetMismatch)));
                } else {
                    assert!(result.is_ok());
                }
                assert_eq!((target.storage.writes, target.resizes, log.writes), writes);
                assert_eq!(log.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
        for at in [
            0,
            case.definition as usize * 4096 + 79,
            case.roots[1] as usize * 4096 + 480,
            case.roots[2] as usize * 4096 + 4095,
            case.data as usize * 4096 + 4095,
        ] {
            let mut forged = case.after.clone();
            forged[at] ^= 1;
            let mut log = Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = log.bytes.get_ref().clone();
            let writes = log.writes;
            let result = if case.action == "insert" {
                let intent =
                    NativeCompleteInsertIntent::decode(&wire(&case), &mut budget(), &|| false)
                        .unwrap();
                let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                qualify_native_complete_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap()
            } else {
                let intent =
                    NativeDeleteIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
                let checked = inspect_v1(&mut log, &mut budget(), &|| false).unwrap();
                qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            };
            assert!(matches!(
                result,
                NativeRecoveryQualificationError::ImageMismatch
            ));
            assert_eq!(log.writes, writes);
            assert_eq!(log.bytes.get_ref(), &saved);
            forgeries += 1;
        }
    }
    assert_eq!(contexts, 48);
    assert_eq!(forgeries, 40);
}

#[test]
#[ignore = "requires54 independently qualified Text cardinality source views"]
fn text_cardinality_eager_duplicate_and_exact_stale_controls_publish_no_plan() {
    let mut controls = 0;
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "delete-case-pair")
    {
        let mut stale = case.witnesses().remove(0);
        stale.1[2] ^= 1;
        let error = image(&case.before)
            .plan_recoverable_ordinary_row_deletes(
                case.table,
                vec![RowDelete::new(stale.0, stale.1)],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RecoverableEditError::Edit(EditError::Conflict)
                | RecoverableEditError::Intent(NativeIntentError::Request(EditError::Conflict))
        ));
        let witness = case.witnesses().remove(0);
        assert!(
            image(&case.before)
                .plan_recoverable_ordinary_row_deletes(
                    case.table,
                    vec![
                        RowDelete::new(witness.0, witness.1.clone()),
                        RowDelete::new(witness.0, witness.1)
                    ],
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        let error = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                case.table,
                vec![RowInsert::new(vec![
                    Value::Int32(1),
                    Value::Text("a".into()),
                ])],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            super::planning_inspection(error),
            InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey)
        ));
        assert!(
            image(&case.before)
                .plan_recoverable_ordinary_complete_row_inserts(
                    case.table,
                    vec![RowInsert::new(vec![
                        Value::Int32(25),
                        Value::Text("a".repeat(256))
                    ])],
                    &mut budget(),
                    &|| false
                )
                .is_err()
        );
        controls += 4;
    }
    assert_eq!(controls, 16);
}
