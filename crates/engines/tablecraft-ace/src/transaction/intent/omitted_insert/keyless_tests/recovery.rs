//! Exact original presence regenerates the same keyless plan under checked V3.
use super::{Error, Intent, budget, literal};
use crate::{
    InsertFieldValue, NativeImage, NativeRecoveryQualificationError, OmittedRowInsert,
    qualify_native_omitted_insert_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{field_changes, fields},
    support::io::journal::MemoryStorage,
};

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let root = usize::from(u16::from_le_bytes(
        seed[4 * 4096 + 12..4 * 4096 + 14].try_into().unwrap(),
    ));
    let keyless = field_changes::keyless_first_field(&seed, "Keyless");
    (
        field_changes::drop_field(&keyless, definition, root, "Id"),
        definition,
    )
}

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "whole image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first different byte"
    );
}

#[test]
fn keyless_presence_requalifies_empty_and_real_pending_committed_settled_journals() {
    let (before, definition) = source();
    for value in [None, Some(77)] {
        for empty in [false, true] {
            let rows = if empty {
                Vec::new()
            } else {
                vec![vec![value.map(Value::Int32)]]
            };
            let wire = literal(&before, rows);
            let expected = if empty {
                before.clone()
            } else {
                field_changes::inserting::keyless_score(&before, definition, value)
            };
            for committed in [false, true] {
                let requests = if empty {
                    Vec::new()
                } else {
                    vec![OmittedRowInsert::new(vec![
                        value.map_or(InsertFieldValue::Omitted, |value| {
                            InsertFieldValue::Supplied(Value::Int32(value))
                        }),
                    ])]
                };
                let plan = image(&before, 4621)
                    .plan_recoverable_retained_keyless_insert(
                        "Keyless",
                        requests,
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert!(plan.intent().is_retained_keyless());
                assert_eq!(
                    plan.intent().encode(&mut budget(), &|| false).unwrap(),
                    wire
                );
                assert_image(plan.after_bytes(), &expected);
                let plan = plan
                    .requalify_with_budgets(
                        image(&before, 4622),
                        &mut budget(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                let mut target = MemoryStorage::new(before.clone());
                let mut file = MemoryStorage::new(Vec::new());
                let prepared = plan
                    .prepare_retained(&mut target, &mut file, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                    target.bytes.get_mut().copy_from_slice(&expected);
                }
                let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
                Intent::decode_retained_keyless(&wire, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(4623),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .recover_retained(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_image(
                    target.bytes.get_ref(),
                    if committed { &expected } else { &before },
                );
                let writes = target.writes;
                let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
                Intent::decode_retained_keyless(&wire, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(4624),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .confirm_completed(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(target.writes, writes);
            }
        }
    }
}

#[test]
fn keyless_forged_after_binding_cancel_and_old_protocol_refuse_before_target_effects() {
    let (before, definition) = source();
    let wire = literal(&before, vec![vec![None]]);
    let expected = field_changes::inserting::keyless_score(&before, definition, None);
    let mut target = MemoryStorage::new(before.clone());
    let mut file = MemoryStorage::new(Vec::new());
    let plan = image(&before, 4625)
        .plan_recoverable_retained_keyless_insert(
            "Keyless",
            vec![OmittedRowInsert::new(vec![InsertFieldValue::Omitted])],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(
        plan.prepare(&mut target, &mut file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert_eq!(file.writes, 0);
    for failure in ["after", "binding", "cancel"] {
        let mut after = expected.clone();
        if failure == "after" {
            after[7 * 4096 + 100] ^= 1;
        }
        let mut original = wire.clone();
        if failure == "binding" {
            original[24] ^= 1;
        }
        let mut file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut file,
                &before,
                &after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut file, &mut budget(), &|| false).unwrap();
        let result = Intent::decode_retained_keyless(&original, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(4626), &mut budget(), &|| {
                failure == "cancel"
            });
        assert!(
            matches!(
                (failure, result),
                (
                    "after",
                    Err(NativeRecoveryQualificationError::ImageMismatch)
                ) | (
                    "binding",
                    Err(NativeRecoveryQualificationError::Intent(
                        Error::BeforeMismatch
                    ))
                ) | (
                    "cancel",
                    Err(NativeRecoveryQualificationError::Inspect(
                        crate::InspectError::Cancelled
                    ))
                )
            ),
            "failure {failure}"
        );
        assert_eq!(target.writes, 0);
        assert_image(target.bytes.get_ref(), &before);
    }
    drop(
        journal::prepare_expected(
            &mut target,
            &mut file,
            &before,
            &expected,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_omitted_insert_recovery(
            checked,
            Intent::decode_retained_keyless(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(4627),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            Error::UnsupportedVersion
        ))
    ));
    assert_eq!(target.writes, 0);
}
