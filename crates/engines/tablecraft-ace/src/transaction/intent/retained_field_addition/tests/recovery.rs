//! Fresh source/AFTER reconstruction qualifies only the original V3 ADD session.
use super::literal;
use crate::{
    NativeImage, NativeRecoveryQualificationError,
    NativeRetainedTypedFieldAdditionIntent as Intent,
    RetainedTypedFieldAdditionIntentError as Error,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{fields, typed_fields as raw},
    support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000_000, 1_000_000, 500_000_000)
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

fn equal_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn typed_field_addition_v3_rebuilds_all_twelve_pending_committed_and_settled_images() {
    let source = fields::source(fields::Case::FirstNumber);
    for kind in raw::KINDS {
        let wire = literal(&source.bytes, kind);
        let after = raw::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            "AddedValue",
            kind,
        );
        for committed in [false, true] {
            let intent = Intent::decode(&wire, &mut budget(), &|| false).unwrap();
            let definition = crate::RetainedFieldAdditionDefinition::capture(
                intent.definition().column(),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let plan = image(&source.bytes, 4310)
                .plan_recoverable_retained_typed_field_addition(
                    "Table1",
                    definition,
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(
                plan.intent().encode(&mut budget(), &|| false).unwrap(),
                wire
            );
            equal_image(plan.after_bytes(), &after);
            let plan = plan
                .requalify_with_budgets(
                    image(&source.bytes, 4311),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut target = MemoryStorage::new(source.bytes.clone());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&after);
            }
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let qualified = Intent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(4312), &mut budget(), &|| {
                    false
                })
                .unwrap();
            assert_eq!(
                qualified.state(),
                if committed {
                    ResizableState::Committed
                } else {
                    ResizableState::Pending
                }
            );
            qualified
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            equal_image(
                target.bytes.get_ref(),
                if committed { &after } else { &source.bytes },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            Intent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(4313), &mut budget(), &|| {
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
fn typed_field_addition_stale_original_and_forged_after_refuse_before_target_writes() {
    let source = fields::source(fields::Case::SecondText);
    let wire = literal(&source.bytes, raw::Kind::Currency);
    let after = raw::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "AddedValue",
        raw::Kind::Currency,
    );
    for forged_before in [false, true] {
        let mut before = source.bytes.clone();
        let mut forged_after = after.clone();
        if forged_before {
            let last = before.len() - 1;
            before[last] ^= 1;
        } else {
            let last = forged_after.len() - 1;
            forged_after[last] ^= 1;
        }
        let mut target = MemoryStorage::new(before.clone());
        let mut journal_file = MemoryStorage::new(Vec::new());
        drop(
            journal::prepare_resizable_v3_expected(
                &mut target,
                &mut journal_file,
                &before,
                &forged_after,
                &mut budget(),
                &|| false,
            )
            .unwrap(),
        );
        let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
        let result = Intent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(4314), &mut budget(), &|| false);
        if forged_before {
            assert!(matches!(result, Err(Error::BeforeMismatch)));
        } else {
            assert!(matches!(
                result,
                Err(Error::Proof(
                    NativeRecoveryQualificationError::ImageMismatch
                ))
            ));
        }
        assert_eq!(target.writes, 0);
        equal_image(target.bytes.get_ref(), &before);
    }
}
