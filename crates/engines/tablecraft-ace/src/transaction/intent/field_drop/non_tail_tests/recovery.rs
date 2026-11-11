//! Fresh non-tail reconstruction reuses the exact checked V3 journal session.
use super::{Error, Facts, Intent, budget, literal};
use crate::{
    NativeImage, NativeRecoveryQualificationError, RecoverableFieldDropPlan,
    qualify_native_field_drop_recovery,
};
use std::io::Cursor;
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{field_changes, fields, typed_fields},
    support::io::journal::MemoryStorage,
};

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn source(case: fields::Case) -> fields::Source {
    let mut source = fields::source(case);
    source.bytes = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Amount",
        typed_fields::Kind::Currency,
    );
    if matches!(case, fields::Case::FirstNumber) {
        source.bytes =
            typed_fields::update_currency(&source.bytes, source.definition, 0, Some(12345));
    }
    source
}

fn plan(source: &fields::Source, column: &str) -> RecoverableFieldDropPlan {
    image(&source.bytes, 4501)
        .plan_recoverable_retained_non_tail_field_drop("Table1", column, &mut budget(), &|| false)
        .unwrap()
}

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn fixed_and_variable_middle_fields_rebuild_pending_committed_and_settled_v3() {
    for (case, column) in [
        (fields::Case::FirstNumber, "Score"),
        (fields::Case::SecondText, "Name"),
    ] {
        let source = source(case);
        let after = field_changes::drop_field(
            &source.bytes,
            source.definition,
            source.catalog_root,
            column,
        );
        let wire = literal(
            &source.bytes,
            Facts {
                definition: source.definition,
                number: 1,
                column_id: 1,
            },
            column,
        );
        for committed in [false, true] {
            let reviewed = plan(&source, column);
            assert!(reviewed.intent().is_retained_non_tail());
            assert!(!reviewed.intent().is_retained_typed());
            assert!(!reviewed.intent().is_retained());
            assert_eq!(
                reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
                wire
            );
            assert_image(reviewed.after_bytes(), &after);
            let fresh = reviewed
                .requalify_with_budgets(
                    image(&source.bytes, 4502),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut target = MemoryStorage::new(source.bytes.clone());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = fresh
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&after);
            }
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            Intent::decode_retained_non_tail(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(4503), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_image(
                target.bytes.get_ref(),
                if committed { &after } else { &source.bytes },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            Intent::decode_retained_non_tail(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(4504), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(target.writes, writes);
        }
    }
}

#[test]
fn non_tail_recovery_refuses_forged_after_identity_binding_v1_and_cancel_without_target_effects() {
    let source = source(fields::Case::FirstNumber);
    let reviewed = plan(&source, "Score");
    let wire = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
    let after = field_changes::drop_field(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Score",
    );
    let mut target = MemoryStorage::new(source.bytes.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(matches!(
        reviewed.prepare(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);

    for failure in ["after", "binding", "identity", "cancel"] {
        let mut stored_after = after.clone();
        if failure == "after" {
            stored_after[7 * 4096 + 100] ^= 1;
        }
        let mut original = wire.clone();
        if failure == "binding" {
            original[24] ^= 1;
        } else if failure == "identity" {
            original[66..68].copy_from_slice(&7_u16.to_le_bytes());
        }
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &source.bytes,
                &stored_after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let intent = Intent::decode_retained_non_tail(&original, &mut budget(), &|| false).unwrap();
        let result = intent.qualify_retained_recovery(
            checked,
            CatalogScope::new(4505),
            &mut budget(),
            &|| failure == "cancel",
        );
        assert!(
            matches!(
                (failure, result),
                (
                    "after",
                    Err(Error::Proof(
                        NativeRecoveryQualificationError::ImageMismatch
                    ))
                ) | ("binding", Err(Error::BeforeMismatch))
                    | ("identity", Err(Error::IdentityMismatch))
                    | (
                        "cancel",
                        Err(Error::Inspect(crate::InspectError::Cancelled))
                    )
            ),
            "failure {failure}"
        );
        assert_eq!(target.writes, 0);
        assert_image(target.bytes.get_ref(), &source.bytes);
        assert!(!journal_file.bytes.get_ref().is_empty());
    }
    let mut original_journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut original_journal,
            &source.bytes,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut original_journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_field_drop_recovery(
            checked,
            Intent::decode_retained_non_tail(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(4506),
            &mut budget(),
            &|| false
        ),
        Err(Error::InvalidRecord)
    ));
    assert_eq!(target.writes, 0);
    assert_image(target.bytes.get_ref(), &source.bytes);
}
