//! TCNA2 retains omitted presence; keys come from fresh source-bound Increment.
#[path = "normal_autonumber/recovery.rs"]
mod recovery;
#[path = "normal_autonumber/resources.rs"]
mod resources;

use super::{Storage, budget, scope};
use sha2::{Digest, Sha256};
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexConstraintViolation, InsertFieldValue, InspectError, NativeImage,
    NativeOmittedInsertIntent, OmittedRowInsert, RecoverableEditError, RowInsert,
    inspect_table_index_counter, read_table, read_table_index,
};
use tablecraft_core::values::Value;
use tablecraft_testkit::fixtures::access::genuine_normal_autonumber as fixtures;

fn sources() -> fixtures::Sources {
    fixtures::Sources::load(|raw, wanted| {
        let actual = Sha256::digest(raw)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, wanted);
    })
}

fn image(raw: &[u8]) -> NativeImage {
    NativeImage::capture(&mut Cursor::new(raw), scope(), &mut budget(), &|| false).unwrap()
}

fn requests(empty: bool) -> Vec<OmittedRowInsert> {
    if empty {
        return Vec::new();
    }
    vec![OmittedRowInsert::new(vec![
        InsertFieldValue::Omitted,
        InsertFieldValue::Supplied(Value::Int32(17)),
    ])]
}

fn wire(before: &[u8], empty: bool) -> Vec<u8> {
    let mut raw = Vec::from(*b"TCNA0002");
    raw.extend_from_slice(&(if empty { 73_u32 } else { 101 }).to_le_bytes());
    raw.extend_from_slice(&[2, 0, 0, 0]);
    raw.extend_from_slice(&(before.len() as u64).to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(b"TCNI0001:BEFORE\0");
    digest.update((before.len() as u64).to_le_bytes());
    digest.update(before);
    raw.extend_from_slice(&digest.finalize());
    for field in [5_u32, u32::from(!empty), 0] {
        raw.extend_from_slice(&field.to_le_bytes());
    }
    raw.extend_from_slice(b"Items");
    if !empty {
        raw.extend_from_slice(&28_u32.to_le_bytes());
        raw.extend_from_slice(&2_u32.to_le_bytes());
        raw.extend_from_slice(&[8, 0, 0, 0]);
        raw.extend_from_slice(&0_u32.to_le_bytes());
        raw.extend_from_slice(&[2, 0, 0, 0]);
        raw.extend_from_slice(&4_u32.to_le_bytes());
        raw.extend_from_slice(&17_i32.to_le_bytes());
    }
    raw
}

#[test]
#[ignore = "requires source14 native/SDK qualification before normal generation"]
fn increment_omissions_commit_four_exact_images_and_empty_preserves_last() {
    let sources = sources();
    let mut commits = 0;
    for header in [5, 6] {
        for recipe in fixtures::RECIPES {
            let before = sources.bytes(header, recipe.before());
            let expected = sources.bytes(header, recipe.after());
            let plan = image(before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    requests(false),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.before_bytes(), before);
            assert_eq!(plan.after_bytes(), expected);
            assert_eq!(plan.inserted_rows(), 1);
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(before, false)
            );
            let decoded =
                NativeOmittedInsertIntent::decode(&wire(before, false), &mut budget(), &|| false)
                    .unwrap();
            assert_eq!(
                decoded.encode(&mut budget(), &|| false).unwrap(),
                wire(before, false)
            );
            let mut target = Storage::new(before.to_vec());
            let mut journal = Storage::new(Vec::new());
            assert_eq!(
                plan.prepare(&mut target, &mut journal, &mut budget(), &|| false)
                    .unwrap()
                    .commit(&mut budget(), &|| false)
                    .unwrap(),
                1
            );
            assert_eq!(target.bytes.get_ref().as_slice(), expected);
            let after = image(target.bytes.get_ref());
            assert_eq!(
                read_table(
                    &mut after.reader(),
                    scope(),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                recipe.rows(true)
            );
            assert_eq!(
                read_table_index(
                    &mut after.reader(),
                    scope(),
                    "Items",
                    "PrimaryId",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                recipe.rows(true)
            );
            assert_eq!(
                inspect_table_index_counter(
                    &mut after.reader(),
                    scope(),
                    "Items",
                    "PrimaryId",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .value(),
                recipe.rows(true).len() as u32
            );
            assert_eq!(
                &expected[7 * 4096 + 20..7 * 4096 + 24],
                recipe.generated().to_le_bytes()
            );
            let empty = image(before)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    requests(true),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(empty.after_bytes(), before);
            assert_eq!(empty.inserted_rows(), 0);
            assert_eq!(
                empty.intent().encode(&mut budget(), &|| false).unwrap(),
                wire(before, true)
            );
            commits += 1;
        }
    }
    assert_eq!(commits, 4);
}

#[test]
#[ignore = "requires source14; no Random/unknown or explicit-ID generation"]
fn random_unknown_overflow_explicit_null_and_explicit_id_refuse_before_publication() {
    let sources = sources();
    for header in [5, 6] {
        for name in ["random-special-default", "unknown-generator-default"] {
            let before = sources.bytes(header, name);
            assert_eq!(
                read_table(
                    &mut image(before).reader(),
                    scope(),
                    "Items",
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .rows(),
                fixtures::Recipe::High.rows(false)
            );
            for empty in [false, true] {
                assert!(
                    image(before)
                        .plan_recoverable_ordinary_omitted_row_inserts(
                            "Items",
                            requests(empty),
                            &mut budget(),
                            &|| false
                        )
                        .is_err()
                );
            }
        }
        let maximum = sources.bytes(header, "increment-maximum");
        assert!(matches!(
            image(maximum).plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                requests(false),
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::AutoNumberExhausted))
        ));
        assert_eq!(
            image(maximum)
                .plan_recoverable_ordinary_omitted_row_inserts(
                    "Items",
                    requests(true),
                    &mut budget(),
                    &|| false
                )
                .unwrap()
                .after_bytes(),
            maximum
        );
        let before = sources.bytes(header, fixtures::Recipe::High.before());
        for identity in [Value::Null, Value::Int32(138)] {
            let original = OmittedRowInsert::new(vec![
                InsertFieldValue::Supplied(identity.clone()),
                InsertFieldValue::Supplied(Value::Int32(17)),
            ]);
            assert!(
                image(before)
                    .plan_recoverable_ordinary_omitted_row_inserts(
                        "Items",
                        vec![original],
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
            assert!(
                image(before)
                    .plan_recoverable_ordinary_complete_row_inserts(
                        "Items",
                        vec![RowInsert::new(vec![identity, Value::Int32(17)])],
                        &mut budget(),
                        &|| false
                    )
                    .is_err()
            );
        }
        // Healthy source-relative counter collision: LAST0 must not be repaired
        // from live Id1, and the index must reject generated Id1 atomically.
        let mut collision = before.to_vec();
        collision[7 * 4096 + 20..7 * 4096 + 24].copy_from_slice(&0_i32.to_le_bytes());
        assert_eq!(
            read_table(
                &mut image(&collision).reader(),
                scope(),
                "Items",
                &mut budget(),
                &|| false
            )
            .unwrap()
            .rows(),
            fixtures::Recipe::High.rows(false)
        );
        assert!(matches!(
            image(&collision).plan_recoverable_ordinary_omitted_row_inserts(
                "Items",
                requests(false),
                &mut budget(),
                &|| false
            ),
            Err(RecoverableEditError::Edit(EditError::Inspect(
                InspectError::IndexConstraint(IndexConstraintViolation::DuplicateKey)
            )))
        ));
    }
}
