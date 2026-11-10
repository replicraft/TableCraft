//! Cold TCND3 recovery binds original raw witnesses, not expanded tombstones.
use crate::{
    EditError, NativeDeleteIntent, NativeImage, NativeIntentError,
    NativeRecoveryQualificationError, RecoverableEditError, RowDelete,
    qualify_native_delete_recovery,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_io::journal::{self, JournalError, ResizableState};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::mixed::after_off::deleted_field::populated::{
            Case, readdition::Target as Field,
        },
        canonical_retained_add_table::{
            Target, deletes, desktop,
            rows::{self, Input, Profile},
            updates,
        },
        canonical_table_drop::current::{self as dropped, History},
    },
    support::io::journal::MemoryStorage,
};

fn budget() -> ResourceBudget {
    ResourceBudget::new(512 * 1024 * 1024, 100_000, 500_000_000)
}

fn source(text: bool) -> (Vec<u8>, u32) {
    let old = dropped::source(Case::MixedBinary, Field::ShortText, true, History::Inserted);
    let empty = dropped::after(&old);
    let definition = (empty.len() / 4096) as u32;
    let (target, profile, first, second) = if text {
        (
            Target::GeneratedText,
            Profile::DesktopText,
            Input::Text("first"),
            Input::Text("last"),
        )
    } else {
        (
            Target::GeneratedNumber,
            Profile::DesktopNumber,
            Input::Number(123),
            Input::Number(42),
        )
    };
    let created = desktop::append(&empty, "DeleteTarget", target);
    let one = rows::append(&created, definition, profile, first);
    let two = rows::append(&one, definition, profile, second);
    let before = if text {
        updates::text(&two, definition, 0, Some("日本語🦀"))
    } else {
        two
    };
    (before, definition)
}

fn witnesses(before: &[u8], definition: u32, ordinals: &[usize]) -> Vec<RowDelete> {
    ordinals
        .iter()
        .map(|&row| RowDelete::new(row, deletes::record(before, definition, row).to_vec()))
        .collect()
}

fn image(before: &[u8], scope: u64) -> NativeImage {
    NativeImage::capture_retained(
        &mut Cursor::new(before),
        CatalogScope::new(scope),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

#[test]
fn retained_delete_requalifies_original_order_and_old_prepare_has_no_effects() {
    for text in [false, true] {
        let (before, definition) = source(text);
        let selected = [1, 0];
        let after = deletes::after(&before, definition, &selected);
        let plan = image(&before, 3811)
            .plan_recoverable_retained_row_deletes(
                "DeleteTarget",
                witnesses(&before, definition, &selected),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert!(plan.intent().is_retained());
        let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
        let plan = plan
            .requalify_with_budgets(image(&before, 3812), &mut budget(), &mut budget(), &|| {
                false
            })
            .unwrap();
        assert_eq!(plan.before_bytes(), before);
        assert_eq!(plan.after_bytes(), after);
        assert_eq!(plan.deleted_rows(), 2);
        assert_eq!(plan.intent().deletes()[0].row(), 1);
        assert_eq!(plan.intent().deletes()[1].row(), 0);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire
        );
        let mut target = MemoryStorage::new(before.to_vec());
        let mut storage = MemoryStorage::new(Vec::new());
        assert!(matches!(
            plan.prepare(&mut target, &mut storage, &mut budget(), &|| false),
            Err(JournalError::ImageSize)
        ));
        assert_eq!((target.writes, storage.writes), (0, 0));
        assert!(storage.bytes.get_ref().is_empty());
        assert_eq!(target.bytes.get_ref(), &before);
    }
}

#[test]
fn retained_delete_pending_committed_and_empty_requests_recover_the_same_v3_owner() {
    for text in [false, true] {
        let (before, definition) = source(text);
        for selected in [&[][..], &[1, 0][..]] {
            let after = deletes::after(&before, definition, selected);
            if selected.is_empty() {
                assert_eq!(after, before);
            }
            for committed in [false, true] {
                let plan = image(&before, 3813)
                    .plan_recoverable_retained_row_deletes(
                        "DeleteTarget",
                        witnesses(&before, definition, selected),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                assert_eq!(plan.after_bytes(), after);
                let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
                let mut target = MemoryStorage::new(before.to_vec());
                let mut storage = MemoryStorage::new(Vec::new());
                let prepared = plan
                    .prepare_retained(&mut target, &mut storage, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    assert_eq!(
                        prepared.commit(&mut budget(), &|| false).unwrap(),
                        selected.len()
                    );
                } else {
                    drop(prepared);
                    target.bytes.get_mut().copy_from_slice(&after);
                }
                assert!(journal::inspect_v1(&mut storage, &mut budget(), &|| false).is_err());
                assert!(journal::inspect_v2(&mut storage, &mut budget(), &|| false).is_err());
                let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
                let qualified =
                    NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| false)
                        .unwrap()
                        .qualify_retained_recovery(
                            checked,
                            CatalogScope::new(3814),
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
                    .recover_retained(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_eq!(
                    target.bytes.get_ref(),
                    if committed { &after } else { &before }
                );
                let writes = target.writes;
                let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
                NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(3815),
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
fn retained_delete_rejects_stale_witness_forged_after_and_old_journal_families() {
    let (before, definition) = source(true);
    let mut stale = deletes::record(&before, definition, 0).to_vec();
    stale[2] ^= 1;
    assert!(matches!(
        image(&before, 3816).plan_recoverable_retained_row_deletes(
            "DeleteTarget",
            vec![RowDelete::new(0, stale)],
            &mut budget(),
            &|| false
        ),
        Err(RecoverableEditError::Edit(EditError::Conflict))
    ));
    let wire = NativeDeleteIntent::new_retained(
        "DeleteTarget",
        witnesses(&before, definition, &[0]),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .encode(&mut budget(), &|| false)
    .unwrap();
    let mut forged = deletes::after(&before, definition, &[0]);
    // DELETE must never rewind or otherwise change the native generation counter.
    forged[definition as usize * 4096 + 20] ^= 0x10;
    let mut target = MemoryStorage::new(before.to_vec());
    let mut storage = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut storage,
            &before,
            &forged,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(3817), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    let older = NativeDeleteIntent::new_current(
        "DeleteTarget",
        witnesses(&before, definition, &[0]),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let checked = journal::inspect_v3(&mut storage, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        older.qualify_retained_recovery(checked, CatalogScope::new(3818), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedVersion
        ))
    ));
    let mut old_storage = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut old_storage,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut old_storage, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_delete_recovery(
            checked,
            NativeDeleteIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(3819),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedVersion
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
