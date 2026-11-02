//! One ordered original-presence intent binds generation, two DATA pages and aliases.
#[path = "increment_multi/recovery.rs"]
mod recovery;
#[path = "increment_multi/resources.rs"]
mod resources;

use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InsertFieldValue, InspectError, NativeImage,
    NativeOmittedInsertIntent, OmittedRowInsert, RecoverableEditError, WriteBlocker,
    inspect_table_index_counter, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_increment_multi as fixtures;

fn cases() -> Vec<fixtures::Case> {
    fixtures::load_all(|raw, wanted| {
        assert_eq!(
            Sha256::digest(raw)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            wanted
        );
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(_: &fixtures::Case, empty: bool) -> Vec<OmittedRowInsert> {
    if empty {
        return Vec::new();
    }
    fixtures::presence()
        .into_iter()
        .map(|row| {
            OmittedRowInsert::new(
                row.into_iter()
                    .map(|value| {
                        value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                    })
                    .collect(),
            )
        })
        .collect()
}

fn wire(case: &fixtures::Case, empty: bool) -> Vec<u8> {
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((case.before.len() as u64).to_le_bytes());
    digest.update(&case.before);
    case.wire(digest.finalize()[..].try_into().unwrap(), empty)
}

#[test]
#[ignore = "requires all four native/SDK-qualified original and batch views"]
fn ordered_omissions_generate_two_rows_and_update_each_physical_history_once() {
    let mut commits = 0;
    for case in cases() {
        let plan = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                case.table,
                requests(&case, false),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 2);
        let literal = wire(&case, false);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            literal
        );
        let decoded =
            NativeOmittedInsertIntent::decode(&literal, &mut budget(), &|| false).unwrap();
        assert_eq!(decoded.encode(&mut budget(), &|| false).unwrap(), literal);
        assert_eq!(decoded.rows().len(), 2);

        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            2
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        let saved = image(target.bytes.get_ref());
        assert_eq!(
            read_table(
                &mut saved.reader(),
                scope(),
                case.table,
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            case.rows(true)
        );
        for (index, alias) in fixtures::ALIASES.into_iter().enumerate() {
            assert_eq!(
                read_table_index(
                    &mut saved.reader(),
                    scope(),
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                case.indexed_rows(alias)
            );
            assert_eq!(
                inspect_table_index_counter(
                    &mut saved.reader(),
                    scope(),
                    case.table,
                    alias,
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .value(),
                if index < 2 { 11 } else { 4 }
            );
        }
        assert_eq!(
            i32::from_le_bytes(
                target.bytes.get_ref()[case.definition * 4096 + 20..case.definition * 4096 + 24]
                    .try_into()
                    .unwrap()
            ),
            139
        );

        let empty = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                case.table,
                Vec::new(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.inserted_rows(), 0);
        assert_eq!(
            empty.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case, true)
        );
        commits += 1;
    }
    assert_eq!(commits, 2);
}

#[test]
#[ignore = "qualified original metadata; supplied generator/null guard/collision/exhaustion refuse"]
fn complete_batch_validation_and_signed_seed_fail_before_publication() {
    let mut denied = 0;
    for case in cases() {
        for (row, field, value, expected) in [
            (0, 0, Some(Value::Null), WriteBlocker::Column),
            (1, 0, Some(Value::Int32(139)), WriteBlocker::Column),
            (1, 2, Some(Value::Null), WriteBlocker::IndexedColumn),
            (1, 1, None, WriteBlocker::Column),
            (1, 2, Some(Value::Int64(7)), WriteBlocker::Column),
        ] {
            let mut presence = fixtures::presence();
            presence[row][field] = value;
            let requests = presence
                .into_iter()
                .map(|row| {
                    OmittedRowInsert::new(
                        row.into_iter()
                            .map(|value| {
                                value.map_or(InsertFieldValue::Omitted, InsertFieldValue::Supplied)
                            })
                            .collect(),
                    )
                })
                .collect();
            assert!(matches!(
                image(&case.before).plan_recoverable_ordinary_omitted_row_inserts(
                    case.table,
                    requests,
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::Unsupported(blocker)))
                    if blocker == expected
            ));
            denied += 1;
        }
        let offset = case.definition * 4096 + 20;
        for last in [i32::MAX, i32::MAX - 1] {
            let mut exhausted = case.before.clone();
            exhausted[offset..offset + 4].copy_from_slice(&last.to_le_bytes());
            assert!(matches!(
                image(&exhausted).plan_recoverable_ordinary_omitted_row_inserts(
                    case.table,
                    requests(&case, false),
                    &mut budget(),
                    &|| false
                ),
                Err(RecoverableEditError::Edit(EditError::AutoNumberExhausted))
            ));
            denied += 1;
        }
        let mut collision = case.before.clone();
        collision[offset..offset + 4].copy_from_slice(&0_i32.to_le_bytes());
        assert!(matches!(
            image(&collision).plan_recoverable_ordinary_omitted_row_inserts(
                case.table,
                requests(&case, false),
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Inspect(
                InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey)
            )))
        ));
        denied += 1;
    }
    assert_eq!(denied, 16);
}

#[test]
#[ignore = "original binding and stale target refusal must precede all journal effects"]
fn original_omission_intent_and_stale_target_never_rebind_to_another_generation_seed() {
    use super::model as journals;
    use tablecraft_ace::{
        NativeIntentError, NativeRecoveryQualificationError, qualify_native_omitted_insert_recovery,
    };
    use tablecraft_io::journal::{JournalError, inspect_v1};

    for case in cases() {
        let mut stale = case.before.clone();
        stale[case.definition * 4096 + 20..case.definition * 4096 + 24]
            .copy_from_slice(&136_i32.to_le_bytes());
        let intent =
            NativeOmittedInsertIntent::decode(&wire(&case, false), &mut budget(), &|| false)
                .unwrap();
        assert!(matches!(
            intent.verify_before(&stale, &mut budget(), &|| false),
            Err(NativeIntentError::BeforeMismatch)
        ));
        let plan = image(&case.before)
            .plan_recoverable_ordinary_omitted_row_inserts(
                case.table,
                requests(&case, false),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let mut target = Storage::new(stale.clone());
        let mut journal = Storage::new(Vec::new());
        assert!(matches!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::TargetMismatch)
        ));
        assert_eq!(target.bytes.get_ref(), &stale);
        assert_eq!(target.writes, 0);
        assert_eq!(journal.writes, 0);
        assert!(journal.bytes.get_ref().is_empty());

        // The exact same original journal is rejected for reversed original
        // omission rows; generated IDs cannot be rebound to physical slot order.
        let mut reversed = requests(&case, false);
        reversed.reverse();
        let intent = NativeOmittedInsertIntent::new(
            case.table,
            reversed,
            &case.before,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut journal = Storage::new(journals::journal(
            &case.before,
            &case.after,
            journals::Marker::Pending,
        ));
        let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
        assert!(matches!(
            qualify_native_omitted_insert_recovery(
                checked,
                intent,
                scope(),
                &mut budget(),
                &|| false
            ),
            Err(NativeRecoveryQualificationError::ImageMismatch)
        ));
        assert_eq!(journal.writes, 0);
    }
}
