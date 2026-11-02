//! Complete literal inline Memo/OLE images, original TCNA and qualified replay.
#[path = "genuine_inline_lval_insert/resources.rs"]
mod resources;

use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, NativeImage, NativeInsertIntent, NativeRecoveryQualificationError,
    RecoverableEditError, RowInsert, WriteBlocker, qualify_native_insert_recovery, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::{RecoveryOutcome, inspect_v1};
use tablecraft_testkit::fixtures::access::genuine_inline_lval_insert as sources;

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

fn requests(recipe: &str) -> Vec<RowInsert> {
    vec![RowInsert::new(sources::values(recipe))]
}

fn wire(before: &[u8], values: &[Value]) -> Vec<u8> {
    let mut row = Vec::from(3_u32.to_le_bytes());
    for value in values {
        let (tag, payload) = match value {
            Value::Null => (0_u8, Vec::new()),
            Value::Int32(value) => (2, value.to_le_bytes().to_vec()),
            Value::Text(value) => (6, value.as_bytes().to_vec()),
            Value::Binary(value) => (7, value.clone()),
            _ => panic!("outside frozen original carriers"),
        };
        row.extend_from_slice(&[tag, 0, 0, 0]);
        row.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        row.extend_from_slice(&payload);
    }
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&((84 + row.len()) as u32).to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((before.len() as u64).to_le_bytes());
    binding.update(before);
    raw.extend_from_slice(&binding.finalize());
    for value in [12_u32, 1, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(b"InlineValues");
    raw.extend_from_slice(&((4 + row.len()) as u32).to_le_bytes());
    raw.extend_from_slice(&row);
    assert_eq!(raw.len(), 84 + row.len());
    raw
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL views"]
fn inline_memo_ole_commits_match_all_literal_images_and_original_carriers() {
    for case in cases() {
        let values = sources::values(case.recipe);
        let plan = image(&case.before)
            .plan_recoverable_ordinary_row_inserts(
                "InlineValues",
                requests(case.recipe),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 1);
        assert_eq!(plan.intent().rows()[0].values(), values);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case.before, &values)
        );
        let mut target = Storage::new(case.before.clone());
        let mut journal = Storage::new(Vec::new());
        assert_eq!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap()
                .commit(&mut budget(), &|| false)
                .unwrap(),
            1
        );
        assert_eq!(target.bytes.get_ref(), &case.after);
        let rows = read_table(
            &mut Cursor::new(target.bytes.get_ref()),
            scope(),
            "InlineValues",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(rows.rows(), sources::rows(case.recipe));
    }
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL views"]
fn inline_memo_ole_recovery_reconstructs_independently_and_rejects_forged_content() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.recipe, "ascii" | "null" | "raw-limit"))
    {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let original = wire(&case.before, &sources::values(case.recipe));
            let mut target = Storage::new(case.after.clone());
            if matches!(marker, Marker::Pending) {
                target.bytes = Cursor::new(case.before.clone());
            } else if matches!(marker, Marker::Partial) {
                target.bytes = Cursor::new(case.before.clone());
                let at = case.definition as usize * 4096 + 16;
                target.bytes.get_mut()[at..at + 4].copy_from_slice(&2_u32.to_le_bytes());
            }
            let expected = if matches!(marker, Marker::Committed) {
                &case.after
            } else {
                &case.before
            };
            let wanted = if matches!(marker, Marker::Committed) {
                RecoveryOutcome::Committed
            } else {
                RecoveryOutcome::RolledBack
            };
            let mut journal = Storage::new(journals::journal(&case.before, &case.after, marker));
            for _ in 0..2 {
                let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
                let intent =
                    NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
                let qualified = qualify_native_insert_recovery(
                    checked,
                    intent,
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(
                    qualified
                        .recover(&mut target, &mut budget(), &|| false)
                        .unwrap(),
                    wanted
                );
                assert_eq!(target.bytes.get_ref(), expected);
            }
            let writes = target.writes;
            target.fail_prefix = Some(0);
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent = NativeInsertIntent::decode(&original, &mut budget(), &|| false).unwrap();
            let qualified =
                qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(
                qualified
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap(),
                wanted
            );
            assert_eq!(target.writes, writes);
            assert_eq!(journal.writes, 0);
        }
        for offset in [
            case.data as usize * 4096 + 4095,
            case.definition as usize * 4096 + 16,
            24,
        ] {
            let mut forged = case.after.clone();
            forged[offset] ^= 1;
            let mut journal =
                Storage::new(journals::journal(&case.before, &forged, Marker::Committed));
            let target = Storage::new(forged);
            let before_target = target.bytes.get_ref().clone();
            let before_journal = journal.bytes.get_ref().clone();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let intent = NativeInsertIntent::decode(
                &wire(&case.before, &sources::values(case.recipe)),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert!(matches!(
                qualify_native_insert_recovery(checked, intent, scope(), &mut budget(), &|| false),
                Err(NativeRecoveryQualificationError::ImageMismatch)
            ));
            assert_eq!(target.bytes.get_ref(), &before_target);
            assert_eq!(journal.bytes.get_ref(), &before_journal);
        }
    }
}

#[test]
#[ignore = "requires eighteen native/SDK-qualified inline LVAL views"]
fn oversize_inline_requests_refuse_without_external_fallback_and_empty_keeps_source() {
    for case in cases().into_iter().filter(|case| case.recipe == "ascii") {
        for values in [
            vec![Value::Int32(2), Value::Text("a".repeat(63)), Value::Null],
            vec![Value::Int32(2), Value::Text("β".repeat(33)), Value::Null],
            vec![Value::Int32(2), Value::Null, Value::Binary(vec![0; 65])],
            vec![Value::Int32(2), Value::Binary(vec![]), Value::Null],
        ] {
            let error = image(&case.before)
                .plan_recoverable_ordinary_row_inserts(
                    "InlineValues",
                    vec![RowInsert::new(values)],
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                RecoverableEditError::Edit(EditError::Unsupported(WriteBlocker::Column))
            ));
        }
        let plan = image(&case.before)
            .plan_recoverable_ordinary_row_inserts(
                "InlineValues",
                Vec::new(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.after_bytes(), case.before);
        assert_eq!(plan.inserted_rows(), 0);
    }
}
