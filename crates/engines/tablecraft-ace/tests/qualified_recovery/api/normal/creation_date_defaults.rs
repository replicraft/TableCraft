//! Original presence remains TCNA2; DATE results come from fresh declaration proof.
#[path = "creation_date_defaults/recovery.rs"]
mod recovery;
#[path = "creation_date_defaults/resources.rs"]
mod resources;

use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{InsertFieldValue, NativeImage, OmittedRowInsert, read_table};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::creation_date_defaults as fixtures;

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn requests(phase: &str) -> Vec<OmittedRowInsert> {
    vec![OmittedRowInsert::new(match phase {
        "omitted" => vec![
            InsertFieldValue::Supplied(Value::Int32(2)),
            InsertFieldValue::Omitted,
        ],
        "supplied-null" => vec![
            InsertFieldValue::Supplied(Value::Int32(3)),
            InsertFieldValue::Supplied(Value::Null),
        ],
        _ => panic!("closed source presence request"),
    })]
}

fn wire(before: &[u8], phase: Option<&str>) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0002");
    raw.extend_from_slice(&(if phase.is_some() { 101_u32 } else { 73 }).to_le_bytes());
    raw.extend_from_slice(&[2, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    for field in [5_u32, u32::from(phase.is_some()), 0] {
        raw.extend_from_slice(&field.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    if let Some(phase) = phase {
        raw.extend_from_slice(&28_u32.to_le_bytes());
        raw.extend_from_slice(&2_u32.to_le_bytes());
        raw.extend_from_slice(&[2, 0, 0, 0]);
        raw.extend_from_slice(&4_u32.to_le_bytes());
        raw.extend_from_slice(&(if phase == "omitted" { 2_i32 } else { 3 }).to_le_bytes());
        raw.extend_from_slice(&[if phase == "omitted" { 8 } else { 0 }, 0, 0, 0]);
        raw.extend_from_slice(&0_u32.to_le_bytes());
    }
    raw
}

#[test]
#[ignore = "requires source36 native parser/bit/Source gates before matching producers"]
fn exact_date_omissions_and_supplied_null_commit_twenty_four_whole_images() {
    let sources = sources();
    let mut commits = 0;
    for header in [5, 6] {
        for case in fixtures::CASES {
            let before = sources.bytes(header, case, "before");
            for phase in ["omitted", "supplied-null"] {
                let expected = sources.bytes(header, case, phase);
                let image = NativeImage::capture(
                    &mut Cursor::new(&before),
                    scope(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                let plan = image
                    .plan_recoverable_ordinary_omitted_row_inserts(
                        "Items",
                        requests(phase),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.before_bytes(), before);
                assert_eq!(plan.after_bytes(), expected);
                assert_eq!(plan.inserted_rows(), 1);
                assert_eq!(
                    plan.intent().encode(&mut budget(), &|| false).unwrap(),
                    wire(&before, Some(phase))
                );
                let mut target = Storage::new(before.clone());
                let mut journal = Storage::new(Vec::new());
                assert_eq!(
                    plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                        .unwrap()
                        .commit(&mut budget(), &|| false)
                        .unwrap(),
                    1
                );
                assert_eq!(target.bytes.get_ref(), &expected);
                assert_eq!(
                    read_table(
                        &mut Cursor::new(target.bytes.get_ref()),
                        scope(),
                        "Items",
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
                    .rows(),
                    fixtures::expected_rows(case, phase)
                );
                commits += 1;
            }
            let image =
                NativeImage::capture(&mut Cursor::new(&before), scope(), &mut budget(), &|| false)
                    .unwrap();
            let empty = image
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    vec![],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(empty.before_bytes(), before);
            assert_eq!(empty.after_bytes(), before);
            assert_eq!(empty.inserted_rows(), 0);
            assert_eq!(
                empty.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(&before, None)
            );
        }
    }
    assert_eq!(commits, 24);
}

#[test]
#[ignore = "requires pinned DATE sources; malformed literal copies have no expected AFTER"]
fn invalid_date_metadata_and_clock_calls_refuse_before_plan_publication() {
    let sources = sources();
    for header in [5, 6] {
        let case = fixtures::CASES[0];
        let before = sources.bytes(header, case, "before");
        let original = case
            .expression
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let matches = before
            .windows(original.len())
            .enumerate()
            .filter_map(|(at, bytes)| (bytes == original.as_slice()).then_some(at))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        for text in [
            "#1900-02-29 00:00:00#",
            "#0099-01-01 00:00:00#",
            "#2024-02-29 24:00:00#",
            "#2024-02-29 23:59:60#",
            "Date()",
            "Now()",
        ] {
            let padded = format!("{text:<21}");
            let replacement = padded
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>();
            assert_eq!(replacement.len(), original.len());
            let mut changed = before.clone();
            changed[matches[0]..matches[0] + replacement.len()].copy_from_slice(&replacement);
            assert_eq!(
                read_table(
                    &mut Cursor::new(&changed),
                    scope(),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                fixtures::expected_rows(case, "before")
            );
            let image =
                NativeImage::capture(&mut Cursor::new(&changed), scope(), &mut budget(), &|| {
                    false
                })
                .unwrap();
            let error = image
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    requests("omitted"),
                    &mut budget(),
                    &|| false,
                )
                .err()
                .unwrap();
            assert!(matches!(
                error,
                tablecraft_ace::RecoverableEditError::Edit(tablecraft_ace::EditError::Unsupported(
                    tablecraft_ace::WriteBlocker::Properties
                ))
            ));
            assert_eq!(before, sources.bytes(header, case, "before"));
        }
    }
}
