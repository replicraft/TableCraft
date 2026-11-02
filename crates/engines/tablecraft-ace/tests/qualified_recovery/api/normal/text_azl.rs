//! Explicit true/false property INSERT through the existing complete service.
#[path = "text_azl/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, resizable_storage::Target, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    ConstraintViolation, InspectError, NativeCompleteInsertIntent, NativeImage,
    NativeRecoveryQualificationError, RecoverableEditError, RowInsert, inspect_table_index_counter,
    qualify_native_complete_insert_recovery, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_image};
use tablecraft_schema::FieldConstraintViolation;
use tablecraft_testkit::access::genuine_text_azl::{self as sources, Case};

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

fn requests(case: &Case) -> Vec<RowInsert> {
    case.insert_values()
        .into_iter()
        .map(RowInsert::new)
        .collect()
}

fn wire(case: &Case) -> Vec<u8> {
    let rows = case.insert_values();
    let mut body = Vec::new();
    for row in &rows {
        let mut payload = Vec::from(2_u32.to_le_bytes());
        for value in row {
            let (tag, raw) = match value {
                Value::Null => (0, Vec::new()),
                Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
                Value::Text(value) => (6, value.as_bytes().to_vec()),
                _ => panic!("closed source carrier"),
            };
            payload.extend_from_slice(&[tag, 0, 0, 0]);
            payload.extend_from_slice(&(raw.len() as u32).to_le_bytes());
            payload.extend_from_slice(&raw);
        }
        body.extend_from_slice(&((payload.len() + 4) as u32).to_le_bytes());
        body.extend_from_slice(&payload);
    }
    let total = 68 + case.table.len() + body.len();
    let mut result = Vec::from(*b"TCNA0004");
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&[4, 0, 0, 0]);
    result.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    result.extend_from_slice(&digest.finalize());
    for word in [case.table.len() as u32, rows.len() as u32, 0] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.extend_from_slice(case.table.as_bytes());
    result.extend_from_slice(&body);
    assert_eq!(result.len(), total);
    result
}

fn intent(case: &Case) -> NativeCompleteInsertIntent {
    NativeCompleteInsertIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

fn empty_text(error: RecoverableEditError) {
    assert!(matches!(
        super::planning_inspection(error),
        InspectError::Constraint(ConstraintViolation::Field(
            FieldConstraintViolation::EmptyText
        ))
    ));
}

#[test]
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn explicit_azl_complete_inserts_retain_literal_images_original_wire_and_typed_false_refusals() {
    let all = cases()
        .into_iter()
        .map(|case| {
            let expected = wire(&case);
            (case, expected)
        })
        .collect::<Vec<_>>();
    let mut commits = 0;
    let mut denied = 0;
    for (case, expected_wire) in all {
        let result = image(&case.before).plan_recoverable_ordinary_complete_row_inserts(
            case.table,
            requests(&case),
            &mut budget(),
            &|| false,
        );
        if case.after.is_none() {
            empty_text(result.err().unwrap());
            denied += 1;
            continue;
        }
        let plan = result.unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after_bytes());
        assert_eq!(plan.inserted_rows(), case.changed_rows());
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            expected_wire
        );
        let mut target = Target::new(case.before.clone());
        let mut log = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare_insert(&mut target, &mut log, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            case.changed_rows()
        );
        assert_eq!(target.storage.bytes.get_ref(), case.after_bytes());
        assert_eq!(target.resizes, 0);
        let read = read_table(
            &mut Cursor::new(case.after_bytes()),
            scope(),
            case.table,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(read.rows(), case.final_rows());
        let asc = read_table_index(
            &mut Cursor::new(case.after_bytes()),
            scope(),
            case.table,
            "Asc",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let twin = read_table_index(
            &mut Cursor::new(case.after_bytes()),
            scope(),
            case.table,
            "Twin",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(asc.rows(), twin.rows());
        assert_eq!(asc.rows().len(), case.final_rows().len());
        for name in ["Primary", "Asc", "Desc"] {
            let expected = if name == "Primary" {
                3 + case.changed_rows()
            } else {
                2 + usize::from(case.recipe == "insert-empty")
            };
            let count = inspect_table_index_counter(
                &mut Cursor::new(case.after_bytes()),
                scope(),
                case.table,
                name,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(count.value() as usize, expected);
        }
        let writes = (target.storage.writes, target.resizes, log.writes);
        let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
        qualify_native_complete_insert_recovery(
            checked,
            intent(&case),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap()
        .confirm_completed(&mut target, &mut budget(), &|| false)
        .unwrap();
        assert_eq!((target.storage.writes, target.resizes, log.writes), writes);
        commits += 1;
    }
    assert_eq!((commits, denied), (20, 4));
}

#[test]
#[ignore = "requires14 independently qualified explicit-AZL source views"]
fn explicit_property_inserts_recover_causal_origins_and_deny_property_and_payload_forgeries() {
    let mut contexts = 0;
    let mut denied = 0;
    for case in cases().into_iter().filter(|case| {
        case.recipe == "insert-empty" && case.after.is_some()
            || case.recipe == "insert-null" && case.table.ends_with("Deny")
    }) {
        let after = case.after_bytes();
        let mut mixed = case.before.clone();
        let base = case.data as usize * 4096;
        mixed[base..base + 4096].copy_from_slice(&after[base..base + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, after);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            for initial in [after.to_vec(), mixed.clone()] {
                let refuse = matches!(marker, Marker::Committed) && initial == mixed;
                let saved = journals::journal(&case.before, after, marker);
                let mut log = Storage::new(saved.clone());
                let mut target = Target::new(initial.clone());
                for _ in 0..2 {
                    let writes = (target.storage.writes, target.resizes, log.writes);
                    let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_complete_insert_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let result = qualified.recover(&mut target, &mut budget(), &|| false);
                    if refuse {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.storage.bytes.get_ref(), &initial);
                        assert_eq!((target.storage.writes, target.resizes, log.writes), writes);
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
                            if committed { after } else { &case.before }
                        );
                    }
                }
                let writes = (target.storage.writes, target.resizes, log.writes);
                let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                let result = qualify_native_complete_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false);
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
        for offset in [
            0,
            case.property_name_offset,
            case.definition as usize * 4096 + 79,
            case.roots[1] as usize * 4096 + 480,
            case.roots[2] as usize * 4096 + 4095,
            case.data as usize * 4096 + 4095,
        ] {
            let mut forged = after.to_vec();
            forged[offset] ^= 1;
            let mut log = Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = log.bytes.get_ref().clone();
            let writes = log.writes;
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_complete_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(log.writes, writes);
            assert_eq!(log.bytes.get_ref(), &saved);
            denied += 1;
        }
    }
    assert_eq!((contexts, denied), (48, 48));
}
