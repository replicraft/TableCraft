//! Literal Binary INSERT images, original request wire and same-token recovery.
#[path = "genuine_binary_indexed/resources.rs"]
mod resources;

use super::{Storage, budget, resizable_storage::Target, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeCompleteInsertIntent, NativeImage, NativeInsertIntent,
    NativeRecoveryQualificationError, RowInsert, WriteBlocker,
    qualify_native_complete_insert_recovery, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_image, prepare_expected};
use tablecraft_testkit::access::genuine_binary_indexed as sources;

fn cases() -> Vec<sources::Case> {
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

fn requests(case: &sources::Case) -> Vec<RowInsert> {
    sources::insert_values(case.recipe)
        .into_iter()
        .map(RowInsert::new)
        .collect()
}

fn wire(case: &sources::Case) -> Vec<u8> {
    let rows = sources::insert_values(case.recipe);
    let mut body = Vec::new();
    for row in &rows {
        let mut payload = Vec::from(3_u32.to_le_bytes());
        for value in row {
            let (tag, bytes) = match value {
                Value::Null => (0, Vec::new()),
                Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
                Value::Binary(value) => (7, value.clone()),
                _ => panic!("outside frozen complete Binary request profile"),
            };
            payload.extend_from_slice(&[tag, 0, 0, 0]);
            payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            payload.extend_from_slice(&bytes);
        }
        body.extend_from_slice(&((4 + payload.len()) as u32).to_le_bytes());
        body.extend_from_slice(&payload);
    }
    let total = 68 + "BinaryKeys".len() + body.len();
    let mut result = Vec::from(*b"TCNA0004");
    result.extend_from_slice(&(total as u32).to_le_bytes());
    result.extend_from_slice(&[4, 0, 0, 0]);
    result.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    result.extend_from_slice(&digest.finalize());
    for value in ["BinaryKeys".len() as u32, rows.len() as u32, 0] {
        result.extend_from_slice(&value.to_le_bytes());
    }
    result.extend_from_slice(b"BinaryKeys");
    result.extend_from_slice(&body);
    assert_eq!(result.len(), total);
    result
}

fn intent(case: &sources::Case) -> NativeCompleteInsertIntent {
    NativeCompleteInsertIntent::decode(&wire(case), &mut budget(), &|| false).unwrap()
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified Binary source views"]
fn fitting_binary_inserts_preserve_six_aliases_literal_images_and_original_requests() {
    // Materialize every independent request wire from the pinned originals and
    // literal requests before the first matching native planner is invoked.
    let literals = cases()
        .into_iter()
        .map(|case| {
            let expected_wire = wire(&case);
            (case, expected_wire)
        })
        .collect::<Vec<_>>();
    let mut committed = 0;
    for (case, expected_wire) in literals {
        let original = sources::insert_values(case.recipe);
        let plan = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                "BinaryKeys",
                requests(&case),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), original.len());
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            expected_wire
        );
        assert_eq!(
            plan.intent()
                .rows()
                .iter()
                .map(RowInsert::values)
                .collect::<Vec<_>>(),
            original.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        );
        assert!(NativeInsertIntent::decode(&expected_wire, &mut budget(), &|| false).is_err());

        let mut target = Target::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare_insert(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            original.len(),
        );
        assert_eq!(target.storage.bytes.get_ref(), &case.after);
        assert_eq!(&journal.bytes.get_ref()[..8], b"TCIMG001");
        assert_eq!(target.resizes, 0);
        let mut expected = sources::original_rows();
        expected.extend(original);
        let observed = read_table(
            &mut Cursor::new(target.storage.bytes.get_ref()),
            scope(),
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(observed.rows(), expected);
        for (left, right) in [
            ("PrimaryID", "PrimaryAlias"),
            ("Ascending", "AscendingAlias"),
            ("Descending", "DescendingAlias"),
        ] {
            let first = read_table_index(
                &mut Cursor::new(target.storage.bytes.get_ref()),
                scope(),
                "BinaryKeys",
                left,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let alias = read_table_index(
                &mut Cursor::new(target.storage.bytes.get_ref()),
                scope(),
                "BinaryKeys",
                right,
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(first.rows(), alias.rows());
            assert_eq!(first.rows().len(), expected.len());
        }
        committed += 1;
    }
    assert_eq!(committed, 16);
}

fn journal(case: &sources::Case, after: &[u8], marker: usize) -> Storage {
    let mut target = Target::new(case.before.clone());
    let mut log = Storage::new(Vec::new());
    let prepared = prepare_expected(
        &mut target,
        &mut log,
        &case.before,
        after,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    if marker == 2 {
        prepared.commit(&mut budget(), &|| false).unwrap();
    } else {
        drop(prepared);
    }
    if marker == 1 {
        log.bytes.get_mut().extend_from_slice(b"COM");
    }
    log
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified Binary source views"]
fn binary_original_wire_recovery_handles_true_mixed_targets_and_protected_forgeries() {
    let mut contexts = 0;
    let mut protected = 0;
    for case in cases() {
        let td = case.definition as usize * 4096;
        let mut mixed = case.before.clone();
        mixed[td..td + 4096].copy_from_slice(&case.after[td..td + 4096]);
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for marker in [0, 1, 2] {
            for initial in [case.after.clone(), mixed.clone()] {
                let refuse = marker == 2 && initial == mixed;
                let mut target = Target::new(initial.clone());
                let mut log = journal(&case, &case.after, marker);
                let saved = log.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_complete_insert_recovery(
                        checked,
                        intent(&case),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let before = (target.storage.writes, target.resizes);
                    let result = qualified.recover(&mut target, &mut budget(), &|| false);
                    if refuse {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.storage.bytes.get_ref(), &initial);
                        assert_eq!((target.storage.writes, target.resizes), before);
                    } else {
                        assert_eq!(
                            result.unwrap(),
                            if marker == 2 {
                                RecoveryOutcome::Committed
                            } else {
                                RecoveryOutcome::RolledBack
                            }
                        );
                        assert_eq!(
                            target.storage.bytes.get_ref(),
                            if marker == 2 {
                                &case.after
                            } else {
                                &case.before
                            }
                        );
                    }
                }
                let before = (target.storage.writes, target.resizes);
                let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_complete_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let confirmed = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if refuse {
                    assert!(matches!(confirmed, Err(JournalError::TargetMismatch)));
                } else {
                    assert!(confirmed.is_ok());
                }
                assert_eq!((target.storage.writes, target.resizes), before);
                assert_eq!(log.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
        for at in [
            td + 67 + 12,
            case.roots[1] as usize * 4096 + 480,
            case.roots[2] as usize * 4096 + 4095,
            case.data as usize * 4096 + 4095,
        ] {
            let mut forged = case.after.clone();
            forged[at] ^= 1;
            let mut log = journal(&case, &forged, 0);
            let saved = log.bytes.get_ref().clone();
            let writes = log.writes;
            let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_complete_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch),
            ));
            assert_eq!(log.writes, writes);
            assert_eq!(log.bytes.get_ref(), &saved);
            protected += 1;
        }
        let mut wrong = sources::insert_values(case.recipe);
        let Value::Int32(identity) = &mut wrong[0][0] else {
            panic!("frozen manual primary request");
        };
        *identity += 100;
        let wrong = NativeCompleteInsertIntent::new(
            "BinaryKeys",
            wrong.into_iter().map(RowInsert::new).collect(),
            &case.before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut log = journal(&case, &case.after, 0);
        let saved = log.bytes.get_ref().clone();
        let writes = log.writes;
        let checked = inspect_image(&mut log, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_complete_insert_recovery(
                checked,
                wrong,
                scope(),
                &mut budget(),
                &|| false,
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch),
        ));
        assert_eq!(log.writes, writes);
        assert_eq!(log.bytes.get_ref(), &saved);
        protected += 1;
    }
    assert_eq!(contexts, 96);
    assert_eq!(protected, 80);
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified Binary source views"]
fn binary_empty_inserts_and_original_delete_witnesses_keep_update_closed() {
    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-null")
    {
        let empty = image(&case.before)
            .plan_recoverable_ordinary_complete_row_inserts(
                "BinaryKeys",
                vec![],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.inserted_rows(), 0);
        for selected in [vec![], vec![0]] {
            let witnesses = image(&case.before)
                .prepare_ordinary_row_deletes("BinaryKeys", &selected, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(witnesses.len(), selected.len());
            if let Some(witness) = witnesses.first() {
                assert_eq!(witness.row(), 0);
                // Original slot0: count3, Id1, Marker17, absent Binary at10,
                // stationary variable footer and mask5. No producer is used.
                assert_eq!(
                    witness.expected_bytes(),
                    &[3, 0, 1, 0, 0, 0, 17, 0, 0, 0, 10, 0, 10, 0, 1, 0, 5,]
                );
            }
        }
        assert!(
            image(&case.before)
                .check_native_ordinary_fixed_table("BinaryKeys", &mut budget(), &|| false,)
                .is_err()
        );
    }
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified Binary source views"]
fn binary_original_source_guards_and_primary_constraints_are_not_permission_fallbacks() {
    use super::planning_inspection;
    use tablecraft_ace::{IndexConstraintViolation, InspectError, RecoverableEditError};

    for case in cases()
        .into_iter()
        .filter(|case| case.recipe == "insert-null")
    {
        for (identity, violation) in [
            (Value::Int32(1), IndexConstraintViolation::DuplicateKey),
            (Value::Null, IndexConstraintViolation::NullKey),
        ] {
            let error = image(&case.before)
                .plan_recoverable_ordinary_complete_row_inserts(
                    "BinaryKeys",
                    vec![RowInsert::new(vec![
                        identity,
                        Value::Null,
                        Value::Int32(238),
                    ])],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert_eq!(
                planning_inspection(error),
                InspectError::IndexConstraint(violation)
            );
        }
        for wrong in [Value::Binary(vec![0; 17]), Value::Int32(0)] {
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_complete_row_inserts(
                    "BinaryKeys",
                    vec![RowInsert::new(vec![
                        Value::Int32(14),
                        wrong,
                        Value::Int32(238)
                    ])],
                    &mut budget(),
                    &|| false,
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::IndexedColumn
                ))),
            ));
        }
        // Extended flag8 is refused by the general row reader as well as
        // normal mutation. Retain that first observed reader boundary.
        let mut unsupported = case.before.clone();
        let field = case.definition as usize * 4096 + 63 + 3 * 12 + 25;
        assert_eq!(unsupported[field + 16], 0);
        unsupported[field + 16] = 8;
        assert!(matches!(
            read_table(
                &mut Cursor::new(&unsupported),
                scope(),
                "BinaryKeys",
                &mut budget(),
                &|| false,
            ),
            Err(InspectError::Unsupported),
        ));
        for rows in [vec![], requests(&case)] {
            assert!(matches!(
                image(&unsupported).plan_recoverable_ordinary_complete_row_inserts(
                    "BinaryKeys",
                    rows,
                    &mut budget(),
                    &|| false,
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::ActiveContent
                ))),
            ));
        }

        // The native layout reader extracts type/number/flags at offsets0/5/15
        // and does not consume descriptor magic bytes1..5. The strict selected
        // TD validator independently reconstructs that complete descriptor.
        // Alter only this classified metadata, preserving readable row cells.
        let mut readable = case.before.clone();
        assert_eq!(&readable[field + 1..field + 5], &[0x59, 6, 0, 0]);
        readable[field + 1] ^= 1;
        let observed = read_table(
            &mut Cursor::new(&readable),
            scope(),
            "BinaryKeys",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(observed.rows(), sources::original_rows());
        for rows in [vec![], requests(&case)] {
            assert!(matches!(
                image(&readable).plan_recoverable_ordinary_complete_row_inserts(
                    "BinaryKeys",
                    rows,
                    &mut budget(),
                    &|| false,
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(
                    WriteBlocker::ActiveContent
                ))),
            ));
        }
    }
}
