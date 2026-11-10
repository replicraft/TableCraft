//! Cold recovery requalifies the exact original request and immutable V3 owner.
use super::{Error, Intent, wire, work};
use crate::{
    NativeImage, RecoverableCurrentFieldAdditionPlan,
    qualify_native_current_field_addition_recovery,
};
use std::io::Cursor;
use tablecraft_io::journal::{self, JournalError, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::fields::{self, Case, Target},
    support::io::journal::MemoryStorage,
};

fn image(bytes: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(bytes),
        CatalogScope::new(scope),
        &mut work(),
        &|| false,
    )
    .unwrap()
}

fn plan(bytes: &[u8], target: Target) -> RecoverableCurrentFieldAdditionPlan {
    let image = image(bytes, 1510);
    match target {
        Target::Number => image.plan_recoverable_retained_number_field_addition(
            "Table1",
            target.name(),
            &mut work(),
            &|| false,
        ),
        Target::ShortText => image.plan_recoverable_retained_short_text_field_addition(
            "Table1",
            target.name(),
            255,
            true,
            &mut work(),
            &|| false,
        ),
    }
    .unwrap()
}

#[test]
fn retained_addition_rebuilds_independent_images_and_keeps_legacy_prepare_closed() {
    for case in fields::CASES {
        let source = fields::source(case);
        for target in [Target::Number, Target::ShortText] {
            let expected = fields::after(
                &source.bytes,
                source.definition,
                source.catalog_root,
                target,
            );
            let reviewed = plan(&source.bytes, target);
            assert!(reviewed.intent().is_retained());
            assert_eq!(reviewed.after_bytes(), expected);
            assert_eq!(
                reviewed.intent().encode(&mut work(), &|| false).unwrap(),
                wire(&source.bytes, matches!(target, Target::ShortText))
            );
            let fresh = reviewed
                .requalify_with_budgets(
                    image(&source.bytes, 1511),
                    &mut work(),
                    &mut work(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(fresh.after_bytes(), expected);
            let mut target_storage = MemoryStorage::new(source.bytes.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            assert!(matches!(
                fresh.prepare(&mut target_storage, &mut journal, &mut work(), &|| false),
                Err(JournalError::ImageSize)
            ));
            assert_eq!(target_storage.writes, 0);
            assert_eq!(journal.writes, 0);
            assert!(journal.bytes.get_ref().is_empty());
        }
    }
}

#[test]
fn retained_pending_and_committed_recovery_keep_the_same_v3_session() {
    for (case, target) in [
        (Case::FirstNumber, Target::Number),
        (Case::SecondText, Target::ShortText),
    ] {
        let source = fields::source(case);
        let after = fields::after(
            &source.bytes,
            source.definition,
            source.catalog_root,
            target,
        );
        let encoded = wire(&source.bytes, matches!(target, Target::ShortText));
        for committed in [false, true] {
            let mut storage = MemoryStorage::new(source.bytes.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            let prepared = plan(&source.bytes, target)
                .prepare_retained(&mut storage, &mut journal, &mut work(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut work(), &|| false).unwrap();
            } else {
                drop(prepared);
                storage.bytes.get_mut().copy_from_slice(&after);
            }
            let checked = journal::inspect_v3(&mut journal, &mut work(), &|| false).unwrap();
            let intent = Intent::decode_retained(&encoded, &mut work(), &|| false).unwrap();
            let qualified = intent
                .qualify_retained_recovery(checked, CatalogScope::new(1512), &mut work(), &|| false)
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
                .recover_retained(&mut storage, &mut work(), &|| false)
                .unwrap();
            assert_eq!(
                storage.bytes.get_ref(),
                if committed { &after } else { &source.bytes }
            );
            let writes = storage.writes;
            let checked = journal::inspect_v3(&mut journal, &mut work(), &|| false).unwrap();
            Intent::decode_retained(&encoded, &mut work(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1513), &mut work(), &|| false)
                .unwrap()
                .confirm_completed(&mut storage, &mut work(), &|| false)
                .unwrap();
            assert_eq!(storage.writes, writes);
        }
    }
}

#[test]
fn forged_after_and_wrong_protocol_refuse_before_target_effects() {
    let source = fields::source(Case::SecondText);
    let target = Target::ShortText;
    let mut after = fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        target,
    );
    let last = after.len() - 1;
    after[last] ^= 1;
    let encoded = wire(&source.bytes, true);
    let mut storage = MemoryStorage::new(source.bytes.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut storage,
            &mut journal,
            &source.bytes,
            &after,
            &mut work(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal, &mut work(), &|| false).unwrap();
    let intent = Intent::decode_retained(&encoded, &mut work(), &|| false).unwrap();
    assert!(matches!(
        intent.qualify_retained_recovery(checked, CatalogScope::new(1514), &mut work(), &|| false),
        Err(Error::ImageMismatch)
    ));
    assert_eq!(storage.writes, 0);
    assert_eq!(storage.bytes.get_ref(), &source.bytes);

    let mut old_journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_expected(
            &mut storage,
            &mut old_journal,
            &source.bytes,
            &after,
            &mut work(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v2(&mut old_journal, &mut work(), &|| false).unwrap();
    let intent = Intent::decode_retained(&encoded, &mut work(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_current_field_addition_recovery(
            checked,
            intent,
            CatalogScope::new(1514),
            &mut work(),
            &|| false
        ),
        Err(Error::InvalidRecord)
    ));
    assert_eq!(storage.writes, 0);

    let mut changed = source.bytes;
    changed[last] ^= 1;
    assert!(matches!(
        Intent::decode_retained(&encoded, &mut work(), &|| false)
            .unwrap()
            .verify_before(&changed, &mut work(), &|| false),
        Err(Error::BeforeMismatch)
    ));
}
