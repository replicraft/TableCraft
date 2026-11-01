//! Public original-row INSERT over independent 800-row NODE source images.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeImage, NativeInsertIntent, NativeRecoveryQualificationError, RowInsert,
    inspect_table_index_counter, qualify_native_insert_recovery, read_table, read_table_index,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::genuine_retained_node_insert::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|bytes, expected| {
        let actual = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests() -> Vec<RowInsert> {
    vec![RowInsert::new(sources::values())]
}

fn wire(before: &[u8], empty: bool) -> Vec<u8> {
    let count = usize::from(!empty);
    let total = 73 + 44 * count;
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&(total as u32).to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"TCNI0001:BEFORE\0");
    hash.update((before.len() as u64).to_le_bytes());
    hash.update(before);
    raw.extend_from_slice(&hash.finalize());
    for number in [5_u32, count as u32, 0] {
        raw.extend_from_slice(&number.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    if !empty {
        raw.extend_from_slice(&44_u32.to_le_bytes());
        raw.extend_from_slice(&3_u32.to_le_bytes());
        for value in [801_i32, 400, 1] {
            raw.extend_from_slice(&[2, 0, 0, 0]);
            raw.extend_from_slice(&4_u32.to_le_bytes());
            raw.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(raw.len(), total);
    raw
}

fn intent(case: &Case) -> NativeInsertIntent {
    NativeInsertIntent::decode(&wire(&case.before, false), &mut budget(), &|| false).unwrap()
}

fn assert_rows(raw: &[u8], inserted: bool) {
    let table = read_table(
        &mut Cursor::new(raw),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(table.rows(), sources::rows(inserted, false));
    for (name, guard, physical, history) in [
        ("PrimaryID", false, 0, if inserted { 801 } else { 800 }),
        ("PrimaryAlias", false, 0, if inserted { 801 } else { 800 }),
        ("GroupOrder", true, 1, 7),
        ("GroupAlias", true, 1, 7),
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
        assert_eq!(cursor.rows(), sources::rows(inserted, guard));
        let count = inspect_table_index_counter(
            &mut Cursor::new(raw),
            scope(),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(count.physical_index(), physical);
        assert_eq!(count.value(), history);
    }
}

#[test]
#[ignore = "requires four source-first NODE INSERT images and native/SDK24 qualification"]
fn node_insert_and_empty_commit_literal_images_with_original_tcna_rows() {
    for case in cases() {
        assert_rows(&case.before, false);
        for empty in [false, true] {
            let expected = if empty { &case.before } else { &case.after };
            let plan = image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    "Items",
                    if empty { vec![] } else { requests() },
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), case.before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.inserted_rows(), usize::from(!empty));
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&case.before, empty)
            );
            let decoded =
                NativeInsertIntent::decode(&wire(&case.before, empty), &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(decoded.rows().len(), usize::from(!empty));
            if !empty {
                assert_eq!(decoded.rows()[0].values(), sources::values());
            }
            let mut target = Storage::new(case.before.clone());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                usize::from(!empty)
            );
            assert_eq!(target.bytes.get_ref(), expected);
            assert_eq!(
                journal.bytes.get_ref(),
                &journals::journal(&case.before, expected, Marker::Committed)
            );
            assert_rows(target.bytes.get_ref(), !empty);
        }
    }
}

#[test]
#[ignore = "requires four source-first NODE INSERT images and native/SDK24 qualification"]
fn actual_mixed_data_leaf_parent_and_history_targets_rollback_or_refuse_committed() {
    let mut contexts = 0;
    for case in cases() {
        // Each pattern copies full AFTER pages into BEFORE, and intentionally
        // leaves another changed owner at BEFORE. No partial source is qualified.
        let groups = [
            vec![case.definition],
            vec![case.data],
            vec![case.left[1]],
            vec![case.roots[1]],
            vec![case.definition, case.data, case.right[0]],
        ];
        for pages in groups {
            let mut mixed = case.before.clone();
            for page in pages {
                let start = page * 4096;
                mixed[start..start + 4096].copy_from_slice(&case.after[start..start + 4096]);
            }
            assert_ne!(mixed, case.before);
            assert_ne!(mixed, case.after);
            for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
                let committed = matches!(marker, Marker::Committed);
                let mut target = Storage::new(mixed.clone());
                let mut journal =
                    Storage::new(journals::journal(&case.before, &case.after, marker));
                let saved = journal.bytes.get_ref().clone();
                for _ in 0..2 {
                    let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                    let qualified = qualify_native_insert_recovery(
                        checked,
                        intent(&case),
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
                let writes = target.writes;
                target.fail_prefix = Some(0);
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let confirmed = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
                if committed {
                    assert!(matches!(confirmed, Err(JournalError::TargetMismatch)));
                } else {
                    assert_eq!(confirmed.unwrap(), RecoveryOutcome::RolledBack);
                }
                assert_eq!(target.writes, writes);
                assert_eq!(journal.writes, 0);
                assert_eq!(journal.bytes.get_ref(), &saved);
                contexts += 1;
            }
        }
    }
    assert_eq!(contexts, 30);
}

fn offsets(case: &Case) -> Vec<usize> {
    vec![
        case.roots[1] * 4096 + 481, // full parent value
        case.roots[1] * 4096 + 485, // full parent ROW suffix
        case.roots[1] * 4096 + 489, // original child identity
        case.left[1] * 4096 + 27,   // leaf partition/end bitmap
        case.right[0] * 4096 + 480, // final leaf membership
        case.left[1] * 4096 + 4095, // retained inactive tail
        case.definition * 4096 + 67,
        case.definition * 4096 + 79,
        case.definition * 4096 + 324, // logical alias descriptor
        case.data * 4096 + 2895,      // new row mask
        case.data * 4096 + 4095,      // old sibling payload
    ]
}

#[test]
#[ignore = "requires four source-first NODE INSERT images and native/SDK24 qualification"]
fn fresh_intent_replay_rejects_fences_rows_partition_history_and_complement_forgeries() {
    let mut refused = 0;
    for case in cases() {
        for offset in offsets(&case) {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Pending));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_insert_recovery(
                    checked,
                    intent(&case),
                    scope(),
                    &mut budget(),
                    &|| false
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
            refused += 1;
        }
    }
    assert_eq!(refused, 22);
}

#[path = "genuine_retained_node_insert/resources.rs"]
mod resources;
