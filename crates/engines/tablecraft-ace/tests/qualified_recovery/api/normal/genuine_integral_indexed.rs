//! Source-qualified exact numeric carriers, original intent wires and durable replay.
#[path = "genuine_integral_indexed/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, NativeDeleteIntent, NativeImage,
    NativeInsertIntent, NativeRecoveryQualificationError, RecoverableEditError, RowDelete,
    RowInsert, WriteBlocker, qualify_native_delete_recovery, qualify_native_insert_recovery,
    read_table,
};
use tablecraft_core::values::{Currency, Value};
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_integral_indexed as sources;

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
    sources::insert_values(case.table, case.recipe)
        .into_iter()
        .map(RowInsert::new)
        .collect()
}

fn deletes(case: &sources::Case, reverse: bool) -> Vec<RowDelete> {
    sources::ordinals(case.recipe, reverse)
        .into_iter()
        .map(|slot| RowDelete::new(slot, sources::record(slot)))
        .collect()
}

fn wire(case: &sources::Case, reverse: bool) -> Vec<u8> {
    let mut body = Vec::new();
    let count;
    if case.operation == "insert" {
        let rows = sources::insert_values(case.table, case.recipe);
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
        let selected = sources::ordinals(case.recipe, reverse);
        count = selected.len();
        for slot in selected {
            body.extend_from_slice(&(slot as u64).to_le_bytes());
            body.extend_from_slice(&19_u32.to_le_bytes());
            body.extend_from_slice(&sources::record(slot));
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
#[ignore = "requires 34 native/SDK-qualified integral source views"]
fn integral_currency_insert_delete_commits_match_literal_data_trees_aliases_and_histories() {
    let mut commits = 0;
    for case in cases() {
        let orders: &[bool] = if case.operation == "delete" && case.recipe == "exterior" {
            &[false, true]
        } else {
            &[false]
        };
        for &reverse in orders {
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            if case.operation == "insert" {
                let expected = sources::insert_values(case.table, case.recipe);
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
                let selected = sources::ordinals(case.recipe, reverse);
                let prepared = image(&case.before)
                    .prepare_ordinary_row_deletes(case.table, &selected, &mut budget(), &|| false)
                    .unwrap();
                for actual in prepared {
                    assert_eq!(actual.expected_bytes(), sources::record(actual.row()));
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
            assert_eq!(rows.rows(), sources::rows(&case));
            commits += 1;
        }
    }
    assert_eq!(commits, 36);
}

fn plan_count(case: &sources::Case) -> usize {
    if case.operation == "insert" {
        sources::insert_values(case.table, case.recipe).len()
    } else {
        sources::ordinals(case.recipe, false).len()
    }
}

#[test]
#[ignore = "requires 34 native/SDK-qualified integral source views"]
fn exact_numeric_carriers_primary_constraints_stale_null_bytes_and_empty_keep_boundaries() {
    for case in cases()
        .into_iter()
        .filter(|case| case.operation == "insert" && case.recipe == "new-family")
    {
        for (id, wanted) in [
            (Value::Int32(1), IndexConstraintViolation::DuplicateKey),
            (Value::Null, IndexConstraintViolation::NullKey),
        ] {
            let mut row = sources::value(case.table, 4, Some(0), 68);
            row[0] = id;
            let error = image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    case.table,
                    vec![RowInsert::new(row)],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(
                matches!(error, RecoverableEditError::Edit(EditError::Inspect(InspectError::IndexConstraint(actual))) if actual == wanted)
            );
        }
        let mut wrong = sources::value(case.table, 4, Some(0), 68);
        wrong[1] = if case.table == "CurrencyKeys" {
            Value::Int64(0)
        } else {
            Value::Currency(Currency::from_scaled(0))
        };
        let wrong = image(&case.before)
            .plan_recoverable_ordinary_row_inserts(
                case.table,
                vec![RowInsert::new(wrong)],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            wrong,
            RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::IndexedColumn))
        ));
        let mut inactive = sources::record(1);
        inactive[6] ^= 1;
        let error = image(&case.before)
            .plan_recoverable_ordinary_row_deletes(
                case.table,
                vec![RowDelete::new(1, inactive)],
                &mut budget(),
                &|| false,
            )
            .err()
            .unwrap();
        assert!(matches!(
            error,
            RecoverableEditError::Edit(EditError::Conflict)
        ));
        let insert = image(&case.before)
            .plan_recoverable_ordinary_row_inserts(case.table, Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(insert.after_bytes(), case.before);
        assert_eq!(insert.inserted_rows(), 0);
        let delete = image(&case.before)
            .plan_recoverable_ordinary_row_deletes(case.table, Vec::new(), &mut budget(), &|| false)
            .unwrap();
        assert_eq!(delete.after_bytes(), case.before);
        assert_eq!(delete.deleted_rows(), 0);
    }
}

#[test]
#[ignore = "requires 34 native/SDK-qualified integral source views"]
fn integral_action_recovery_repeats_causal_mixtures_and_refuses_forged_history_or_inactive_bytes() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.recipe, "batch-families" | "exterior"))
    {
        let original = wire(&case, false);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let mut initial = case.after.clone();
            if matches!(marker, Marker::Partial) {
                initial = case.before.clone();
                let base = case.roots[1] as usize * 4096;
                initial[base..base + 4096].copy_from_slice(&case.after[base..base + 4096]);
                assert_ne!(initial, case.before);
                assert_ne!(initial, case.after);
            }
            let mut target = Storage::new(initial);
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let committed = matches!(marker, Marker::Committed);
            let outcome = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let expected = if committed { &case.after } else { &case.before };
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let actual = if case.operation == "insert" {
                    let intent =
                        NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
                    qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| {
                        false
                    })
                    .unwrap()
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap()
                } else {
                    let intent =
                        NativeDeleteIntent::decode(&original, &mut budget(), &|| false).unwrap();
                    qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| {
                        false
                    })
                    .unwrap()
                    .recover(&mut target, &mut budget(), &|| false)
                    .unwrap()
                };
                assert_eq!(actual, outcome);
                assert_eq!(target.bytes.get_ref(), expected);
            }
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let actual = if case.operation == "insert" {
                let intent =
                    NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
                qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .unwrap()
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap()
            } else {
                let intent =
                    NativeDeleteIntent::decode(&original, &mut budget(), &|| false).unwrap();
                qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .unwrap()
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap()
            };
            assert_eq!(actual, outcome);
            assert_eq!(target.writes, writes);
            assert_eq!(journal.writes, 0);
        }
        for at in [
            case.data as usize * 4096 + 4064,
            case.roots[1] as usize * 4096 + 480,
            case.definition as usize * 4096 + 79,
            24,
        ] {
            let mut forged = case.after.clone();
            forged[at] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Committed));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let error = if case.operation == "insert" {
                let intent =
                    NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
                qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            } else {
                let intent =
                    NativeDeleteIntent::decode(&original, &mut budget(), &|| false).unwrap();
                qualify_native_delete_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .err()
                    .unwrap()
            };
            assert!(matches!(
                error,
                NativeRecoveryQualificationError::ImageMismatch
            ));
            assert_eq!(journal.bytes.get_ref(), &saved);
            assert_eq!(journal.writes, 0);
        }
    }
}
