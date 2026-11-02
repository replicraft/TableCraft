//! Public original-intent indexed insertion against six prequalified source views.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InspectError, NativeImage, NativeInsertIntent,
    NativeRecoveryQualificationError, RecoverableEditError, RowInsert, inspect_table_index_counter,
    qualify_native_insert_recovery, read_table, read_table_index,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_indexed_insert::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(bytes), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(count: usize) -> Vec<RowInsert> {
    assert!(count <= 2);
    (0..count)
        .map(|position| RowInsert::new(sources::values(position == 1)))
        .collect()
}

// Independent TCNA0001 framing retains original complete rows, not index keys.
fn wire(before: &[u8], count: usize) -> Vec<u8> {
    let total = 73 + 44 * count;
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&(total as u32).to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    for number in [5_u32, count as u32, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    for row in [[4_i32, 17, 3], [5, 18, 3]].into_iter().take(count) {
        raw.extend_from_slice(&44_u32.to_le_bytes());
        raw.extend_from_slice(&3_u32.to_le_bytes());
        for value in row {
            raw.extend_from_slice(&[2, 0, 0, 0]);
            raw.extend_from_slice(&4_u32.to_le_bytes());
            raw.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(raw.len(), total);
    raw
}

fn intent(case: &Case, count: usize) -> NativeInsertIntent {
    NativeInsertIntent::decode(&wire(&case.before, count), &mut budget(), &|| false).unwrap()
}

fn assert_rows_and_counters(raw: &[u8], inserted: usize) {
    let wanted = sources::rows(inserted);
    let sequential = read_table(
        &mut Cursor::new(raw),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(sequential.rows(), wanted);
    for (name, physical, value) in [
        ("PrimaryID", 0, 3 + inserted),
        ("PrimaryAlias", 0, 3 + inserted),
        ("GroupOrder", 1, if inserted == 0 { 3 } else { 4 }),
        ("GroupAlias", 1, if inserted == 0 { 3 } else { 4 }),
    ] {
        let cursor = read_table_index(
            &mut Cursor::new(raw),
            scope(),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(cursor.rows(), wanted);
        let counter = inspect_table_index_counter(
            &mut Cursor::new(raw),
            scope(),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.physical_index(), physical);
        assert_eq!(counter.value() as usize, value);
    }
}

#[test]
#[ignore = "requires all six source-first indexed INSERT views and SDK36 qualification"]
fn single_and_ordered_batch_commit_literal_images_and_original_tcna_rows() {
    let cases = cases();
    let mut committed = 0;
    for case in cases {
        assert_rows_and_counters(&case.before, 0);
        for (count, expected) in [(1, &case.single), (2, &case.batch)] {
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    "Items",
                    requests(count),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.inserted_rows(), count);
            assert_eq!(plan.intent().table(), "Items");
            assert_eq!(plan.intent().rows().len(), count);
            for (position, row) in plan.intent().rows().iter().enumerate() {
                assert_eq!(row.values(), sources::values(position == 1));
            }
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case.before, count)
            );
            let decoded = intent(&case, count);
            assert_eq!(decoded.rows().len(), count);
            assert_eq!(
                decoded.encode(&mut budget(), &|| false).unwrap(),
                wire(&case.before, count)
            );

            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                count
            );
            assert_eq!(target.bytes.get_ref(), expected);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, expected, Marker::Committed)
            );
            assert_rows_and_counters(target.bytes.get_ref(), count);
            committed += 1;
        }
    }
    assert_eq!(committed, 4);
}

#[test]
#[ignore = "requires all six source-first indexed INSERT views and SDK36 qualification"]
fn empty_originals_and_duplicate_primary_requests_validate_without_publishing_changes() {
    for case in cases() {
        let empty = image(&case.before)
            .plan_recoverable_ordinary_row_inserts("Items", vec![], &mut budget(), &|| false)
            .unwrap();
        assert_eq!(empty.before_bytes(), case.before);
        assert_eq!(empty.after_bytes(), case.before);
        assert_eq!(empty.inserted_rows(), 0);
        assert_eq!(
            empty.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.before, 0)
        );

        let duplicate = RowInsert::new(vec![
            tablecraft_core::values::Value::Int32(1),
            tablecraft_core::values::Value::Int32(17),
            tablecraft_core::values::Value::Int32(3),
        ]);
        assert!(matches!(
            image(&case.before).plan_recoverable_ordinary_row_inserts(
                "Items",
                vec![duplicate],
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Inspect(
                InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey)
            )))
        ));
    }
}

#[test]
#[ignore = "requires all six source-first indexed INSERT views and SDK36 qualification"]
fn fresh_recovery_rejects_index_alias_history_and_protected_byte_forgeries() {
    let mut refused = 0;
    for case in cases() {
        for (count, expected) in [(1, &case.single), (2, &case.batch)] {
            let td = case.definition as usize * 4096;
            let first = case.roots[0] as usize * 4096;
            let second = case.roots[1] as usize * 4096;
            // Source-literal offsets: new Id4 ROW suffix, secondary key,
            // bitmap, inert tail, each history block, alias descriptor, old DATA.
            for offset in [
                first + 512,
                second + 508,
                first + 31,
                first + 4095,
                td + 67,
                td + 79,
                td + 324,
                case.data as usize * 4096 + 4053,
            ] {
                let mut forged = expected.to_vec();
                forged[offset] ^= 1;
                let mut journal =
                    Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
                let saved = journal.bytes.get_ref().clone();
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                assert!(matches!(
                    qualify_native_insert_recovery(
                        checked,
                        intent(&case, count),
                        scope(),
                        &mut budget(),
                        &|| false,
                    ),
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ));
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                refused += 1;
            }
        }
    }
    assert_eq!(refused, 32);
}

#[test]
#[ignore = "requires all six source-first indexed INSERT views and SDK36 qualification"]
fn original_batch_intent_recovers_pending_and_confirms_committed_without_journal_rewrite() {
    for case in cases() {
        for marker in [Marker::Pending, Marker::Committed] {
            let outcome = if matches!(marker, Marker::Committed) {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let expected = if matches!(marker, Marker::Committed) {
                &case.batch
            } else {
                &case.before
            };
            let mut target = Storage::new(case.batch.clone());
            let mut journal = Storage::new(journals::journal(&case.before, &case.batch, marker));
            let saved = journal.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_insert_recovery(
                    checked,
                    intent(&case, 2),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    outcome
                );
                assert_eq!(target.bytes.get_ref(), expected);
            }
            let writes = target.writes;
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_insert_recovery(
                checked,
                intent(&case, 2),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                outcome
            );
            assert_eq!(target.writes, writes);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

#[test]
#[ignore = "requires all six source-first indexed INSERT views and SDK36 qualification"]
fn mixed_data_index_and_definition_targets_recover_or_refuse_without_journal_replacement() {
    for case in cases() {
        // These are authored interrupted targets, not healthy source images or
        // claims about observing partial states within a transport write.
        for pages in [
            vec![case.definition],
            vec![case.data],
            vec![case.roots[0]],
            vec![case.definition, case.data, case.roots[0]],
        ] {
            let mut mixed = case.before.clone();
            for page in pages {
                let start = page as usize * 4096;
                mixed[start..start + 4096].copy_from_slice(&case.batch[start..start + 4096]);
            }
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.batch);

            for marker in [Marker::Pending, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mut target = Storage::new(mixed.clone());
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.batch, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_insert_recovery(
                        checked,
                        intent(&case, 2),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    let result = qualified.recover(&mut target, &mut budget(), &|| false);
                    if committed {
                        assert!(matches!(result, Err(JournalError::TargetMismatch)));
                        assert_eq!(target.bytes.get_ref(), &mixed);
                        assert_eq!(target.writes, 0);
                    } else {
                        assert_eq!(result.unwrap(), RecoveryOutcome::RolledBack);
                        assert_eq!(target.bytes.get_ref(), &case.before);
                    }
                }
                if !committed {
                    let writes = target.writes;
                    target.fail_prefix = Some(0);
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_insert_recovery(
                        checked,
                        intent(&case, 2),
                        scope(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                    assert_eq!(
                        qualified
                            .confirm_completed(&mut target, &mut budget(), &|| false)
                            .unwrap(),
                        RecoveryOutcome::RolledBack
                    );
                    assert_eq!(target.writes, writes);
                }
                assert_eq!(journal.bytes.get_ref(), &saved);
                assert_eq!(journal.writes, 0);
            }
        }
    }
}

#[path = "genuine_indexed_insert/resources.rs"]
mod resources;
