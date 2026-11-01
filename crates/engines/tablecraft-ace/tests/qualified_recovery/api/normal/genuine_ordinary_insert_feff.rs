//! Source-pinned full INSERT commits and self-consistent alias journal refusals.
use super::{Storage, budget, model as journals, scope};
use journals::Marker;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    NativeImage, NativeInsertIntent, NativeRecoveryQualificationError, RowInsert,
    qualify_native_insert_recovery, read_table,
};
use tablecraft_core::values::Value;
use tablecraft_io::journal::inspect_v1;
use tablecraft_testkit::access::genuine_ordinary_insert_feff::{self as sources, Case};

fn cases() -> Vec<Case> {
    sources::load_all(|raw, expected| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, expected);
    })
}

fn wire(case: &Case) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0001");
    raw.extend_from_slice(&118_u32.to_le_bytes());
    raw.extend_from_slice(&[1, 0, 0, 0]);
    raw.extend_from_slice(&(case.before.len() as u64).to_le_bytes());
    let mut binding = Sha256::new();
    binding.update(b"TCNI0001:BEFORE\0");
    binding.update((case.before.len() as u64).to_le_bytes());
    binding.update(&case.before);
    raw.extend_from_slice(&binding.finalize());
    for value in [8_u32, 1, 0] {
        raw.extend_from_slice(&value.to_le_bytes());
    }
    raw.extend_from_slice(b"Sentinel");
    raw.extend_from_slice(&42_u32.to_le_bytes());
    raw.extend_from_slice(&3_u32.to_le_bytes());
    raw.extend_from_slice(&[2, 0, 0, 0, 4, 0, 0, 0, 24, 0, 0, 0]);
    raw.extend_from_slice(&[6, 0, 0, 0, 6, 0, 0, 0, 239, 187, 191, 110, 101, 119]);
    raw.extend_from_slice(&[0; 8]);
    assert_eq!(raw.len(), 118);
    raw
}

#[test]
#[ignore = "requires six independently qualified FEFF source images"]
fn exact_feff_original_intent_commits_escaped_images_and_keeps_the_marker_as_data() {
    for case in cases() {
        let image = NativeImage::capture(
            &mut Cursor::new(&case.before),
            scope(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = image
            .plan_recoverable_ordinary_row_inserts(
                "Sentinel",
                vec![RowInsert::new(sources::values())],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire(&case)
        );
        assert_eq!(plan.after_bytes(), case.after);
        assert_eq!(plan.inserted_rows(), 1);
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
        assert_eq!(
            journal.bytes.get_ref(),
            &journals::journal(&case.before, &case.after, Marker::Committed)
        );
        let table = read_table(
            &mut Cursor::new(target.bytes.get_ref()),
            scope(),
            "Sentinel",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            table.rows()[1],
            vec![
                Value::Int32(24),
                Value::Text("\u{feff}new".into()),
                Value::Null
            ]
        );
    }
}

#[test]
#[ignore = "requires six independently qualified FEFF source images"]
fn all_journal_markers_refuse_readable_alias_after_before_target_effects() {
    for case in cases() {
        for marker in [Marker::Pending, Marker::Partial, Marker::Committed] {
            let mut journal = Storage::new(journals::journal(&case.before, &case.alias, marker));
            let saved = journal.bytes.get_ref().clone();
            let original =
                NativeInsertIntent::decode(&wire(&case), &mut budget(), &|| false).unwrap();
            let checked = inspect_v1(&mut journal, &mut budget(), &|| false).unwrap();
            let error =
                qualify_native_insert_recovery(checked, original, scope(), &mut budget(), &|| {
                    false
                })
                .err()
                .unwrap();
            assert!(matches!(
                error,
                NativeRecoveryQualificationError::ImageMismatch
            ));
            assert_eq!(journal.writes, 0);
            assert_eq!(journal.bytes.get_ref(), &saved);
        }
    }
}
