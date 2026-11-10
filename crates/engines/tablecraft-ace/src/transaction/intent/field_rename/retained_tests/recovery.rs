//! Fresh native reconstruction and cold recovery retain the same V3 owner.
use super::{Error, Intent, budget, wire};
use crate::{
    NativeImage, NativeRecoveryQualificationError, RecoverableFieldRenamePlan,
    qualify_native_field_rename_recovery,
};
use std::io::Cursor;
use tablecraft_io::journal::{self, JournalError, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{
        field_changes as changes,
        fields::{self, Case, Target},
    },
    support::io::journal::MemoryStorage,
};

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn plan(before: &[u8], target: Target, new_name: &str) -> RecoverableFieldRenamePlan {
    let image = image(before, 1601);
    image
        .plan_recoverable_retained_schema_field_rename(
            "Table1",
            target.name(),
            new_name,
            &mut budget(),
            &|| false,
        )
        .unwrap()
}

#[test]
fn retained_schema_original_request_requalifies_raw_complete_images_in_fresh_scope() {
    for case in fields::CASES {
        for target in [Target::Number, Target::ShortText] {
            let source = changes::source(case, target, true, true, true);
            let expected = changes::rename(
                &source.bytes,
                source.definition,
                source.catalog_root,
                target.name(),
                "ExpandedField",
            );
            let reviewed = plan(&source.bytes, target, "ExpandedField");
            assert!(reviewed.intent().is_retained_schema());
            assert_eq!(reviewed.after_bytes(), expected);
            let encoded = reviewed.intent().encode(&mut budget(), &|| false).unwrap();
            assert_eq!(
                encoded,
                wire::rename(
                    &source.bytes,
                    wire::tail(&source.bytes, source.definition),
                    target.name(),
                    "ExpandedField"
                )
            );
            let fresh = reviewed
                .requalify_with_budgets(
                    image(&source.bytes, 1602),
                    &mut budget(),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(fresh.after_bytes(), expected);
            let mut storage = MemoryStorage::new(source.bytes.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            assert!(matches!(
                fresh.prepare(&mut storage, &mut journal, &mut budget(), &|| false),
                Err(JournalError::ImageSize)
            ));
            assert_eq!(storage.writes, 0);
            assert_eq!(journal.writes, 0);
        }
    }
}

#[test]
fn retained_schema_pending_committed_and_confirmed_states_recover_exact_original_action() {
    for (case, target) in [
        (Case::FirstNumber, Target::Number),
        (Case::SecondText, Target::ShortText),
    ] {
        let source = changes::source(case, target, true, true, true);
        for new_name in ["ExpandedField", target.name()] {
            let expected = changes::rename(
                &source.bytes,
                source.definition,
                source.catalog_root,
                target.name(),
                new_name,
            );
            let encoded = wire::rename(
                &source.bytes,
                wire::tail(&source.bytes, source.definition),
                target.name(),
                new_name,
            );
            for committed in [false, true] {
                let mut storage = MemoryStorage::new(source.bytes.clone());
                let mut journal = MemoryStorage::new(Vec::new());
                let prepared = plan(&source.bytes, target, new_name)
                    .prepare_retained(&mut storage, &mut journal, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                    storage.bytes.get_mut().copy_from_slice(&expected);
                }
                let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
                let qualified = Intent::decode_retained_schema(&encoded, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(1603),
                        &mut budget(),
                        &|| false,
                    )
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
                    .recover_retained(&mut storage, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    storage.bytes.get_ref(),
                    if committed { &expected } else { &source.bytes }
                );
                let writes = storage.writes;
                let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
                Intent::decode_retained_schema(&encoded, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(1604),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .confirm_completed(&mut storage, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(storage.writes, writes);
            }
        }
    }
}

#[test]
fn retained_schema_forged_after_and_original_v1_protocol_refuse_before_target_writes() {
    let target = Target::ShortText;
    let source = changes::source(Case::SecondText, target, true, true, true);
    let mut after = changes::rename(
        &source.bytes,
        source.definition,
        source.catalog_root,
        target.name(),
        "ExpandedField",
    );
    *after.last_mut().unwrap() ^= 1;
    let encoded = wire::rename(
        &source.bytes,
        wire::tail(&source.bytes, source.definition),
        target.name(),
        "ExpandedField",
    );
    let mut storage = MemoryStorage::new(source.bytes.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut storage,
            &mut journal,
            &source.bytes,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        Intent::decode_retained_schema(&encoded, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1605), &mut budget(), &|| false),
        Err(Error::Proof(
            NativeRecoveryQualificationError::ImageMismatch
        ))
    ));
    assert_eq!(storage.writes, 0);
    let mut original_journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut storage,
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
        qualify_native_field_rename_recovery(
            checked,
            Intent::decode_retained_schema(&encoded, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(1605),
            &mut budget(),
            &|| false
        ),
        Err(Error::InvalidRecord)
    ));
    assert_eq!(storage.writes, 0);
    assert_eq!(storage.bytes.get_ref(), &source.bytes);
}
