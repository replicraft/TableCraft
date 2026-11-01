//! Independent TCNA requests and full-image association/recovery controls.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeImage, NativeInsertIntent, NativeRecoveryQualificationError, RowInsert,
    qualify_native_insert_recovery, read_table, read_table_index,
};
use tablecraft_io::journal::{JournalError, RecoveryOutcome, inspect_v1};
use tablecraft_testkit::access::genuine_indexed_insert_association::{self as fixtures, Case};

fn cases() -> Vec<Case> {
    fixtures::load_all(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn wire(before: &[u8]) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&380_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((before.len() as u64).to_le_bytes());
    binding.update(before);
    raw.extend_from_slice(&binding.finalize());
    for value in [5_u32, 2, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    for (id, score, text, length) in [
        (10_i32, 17_i32, "L".repeat(200), 252_u32),
        (11, 18, "new".to_owned(), 55),
    ] {
        raw.extend_from_slice(&length.to_le_bytes());
        raw.extend_from_slice(&4_u32.to_le_bytes());
        for value in [id, score, 7] {
            raw.extend_from_slice(&[2, 0, 0, 0]);
            raw.extend_from_slice(&4_u32.to_le_bytes());
            raw.extend_from_slice(&value.to_le_bytes());
        }
        raw.extend_from_slice(&[6, 0, 0, 0]);
        raw.extend_from_slice(&(text.len() as u32).to_le_bytes());
        raw.extend_from_slice(text.as_bytes());
    }
    assert_eq!(raw.len(), 380);
    raw
}

fn intent(before: &[u8]) -> NativeInsertIntent {
    NativeInsertIntent::decode(&wire(before), &mut budget(), &|| false).unwrap()
}

fn readbacks(raw: &[u8]) {
    let rows = read_table(
        &mut Cursor::new(raw),
        scope(),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(rows.rows(), fixtures::rows(true));
    for index in ["PrimaryID", "PrimaryAlias", "GroupOrder", "GroupAlias"] {
        let rows = read_table_index(
            &mut Cursor::new(raw),
            scope(),
            "Items",
            index,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows(), fixtures::cursor_rows(index));
    }
}

#[test]
#[ignore = "requires four source/SDK-qualified cross-page INSERT association views"]
fn original_request_order_commits_exact_cross_page_rows_and_all_aliases() {
    for case in cases() {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let requests = vec![
            RowInsert::new(fixtures::values(false)),
            RowInsert::new(fixtures::values(true)),
        ];
        let plan = image
            .plan_recoverable_ordinary_row_inserts("Items", requests, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(plan.before_bytes(), case.before);
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 2);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.before)
        );
        assert_eq!(plan.intent().rows()[0].values(), fixtures::values(false));
        assert_eq!(plan.intent().rows()[1].values(), fixtures::values(true));
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        let prepared = plan
            .prepare(&mut target, &mut journal, &mut budget(), &|| false)
            .unwrap();
        assert_eq!(prepared.commit(&mut budget(), &|| false).unwrap(), 2);
        assert_eq!(target.bytes.get_ref(), &case.after);
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed)
        );
        readbacks(target.bytes.get_ref());
    }
}

#[test]
#[ignore = "requires four source/SDK-qualified cross-page INSERT association views"]
fn mixed_pages_roll_back_or_refuse_committed_without_journal_effects() {
    for case in cases() {
        let mut mixed = case.before.clone();
        for page in [case.late as usize, case.roots[0]] {
            let start = page * 4096;
            mixed[start..start + 4096].copy_from_slice(&case.after[start..start + 4096]);
        }
        assert_ne!(mixed, case.before);
        assert_ne!(mixed, case.after);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let committed = matches!(marker, Marker::Committed);
            let outcome = if committed {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let expected = if committed { &case.after } else { &case.before };
            let mut target = Storage::new(mixed.clone());
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            let saved = journal.bytes.get_ref().clone();
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_insert_recovery(
                    checked,
                    intent(&case.before),
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
                    assert_eq!(result.unwrap(), outcome);
                    assert_eq!(target.bytes.get_ref(), expected);
                }
            }
            let writes = target.writes;
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let qualified = qualify_native_insert_recovery(
                checked,
                intent(&case.before),
                scope(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let confirmation = qualified.confirm_completed(&mut target, &mut budget(), &|| false);
            if committed {
                assert!(matches!(confirmation, Err(JournalError::TargetMismatch)));
                assert_eq!(target.bytes.get_ref(), &mixed);
            } else {
                assert_eq!(confirmation.unwrap(), outcome);
                assert_eq!(target.bytes.get_ref(), expected);
            }
            assert_eq!(target.writes, writes);
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}

#[test]
#[ignore = "requires four source/SDK-qualified cross-page INSERT association views"]
fn self_consistent_journal_with_swapped_new_row_associations_refuses_before_effects() {
    for case in cases() {
        let mut forged = case.after.clone();
        let base = case.roots[0] * 4096;
        // Original entries1..9 consume81B. The two new key ROW suffixes are
        // at566..570 and575..579; swap only those, preserving both keys/counts.
        let first: [u8; 4] = forged[base + 566..base + 570].try_into().unwrap();
        let second: [u8; 4] = forged[base + 575..base + 579].try_into().unwrap();
        assert_ne!(first, second);
        forged[base + 566..base + 570].copy_from_slice(&second);
        forged[base + 575..base + 579].copy_from_slice(&first);
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let mut journal = Storage::new(journals::journal(&case.before, &forged, marker));
            let saved = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            assert!(matches!(
                qualify_native_insert_recovery(
                    checked,
                    intent(&case.before),
                    scope(),
                    &mut budget(),
                    &|| false,
                ),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}
