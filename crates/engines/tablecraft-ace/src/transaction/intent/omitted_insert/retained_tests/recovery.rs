//! Original request data is reconstructed against the same checked V3 journal.
use super::{budget, literal};
use crate::{
    InsertFieldValue, NativeImage, NativeOmittedInsertIntent, OmittedRowInsert,
    qualify_native_omitted_insert_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal;
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_created_number::mixed::after_off::deleted_field::populated::{
            Case, readdition::Target as Field,
        },
        canonical_retained_add_table::{
            Target, desktop,
            rows::{self, Input, Profile},
        },
        canonical_table_drop::current::{self as dropped, History},
    },
    support::io::journal::MemoryStorage,
};

fn source() -> (Vec<u8>, u32) {
    let original = dropped::source(Case::MixedBinary, Field::ShortText, true, History::Deleted);
    let deleted = dropped::after(&original);
    let definition = (deleted.len() / 4096) as u32;
    (
        desktop::append(&deleted, "Replacement", Target::GeneratedText),
        definition,
    )
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

fn requests(empty: bool) -> Vec<OmittedRowInsert> {
    if empty {
        Vec::new()
    } else {
        vec![OmittedRowInsert::new(vec![
            InsertFieldValue::Omitted,
            InsertFieldValue::Supplied(Value::Text("日本語🦀".into())),
        ])]
    }
}

#[test]
fn retained_insert_original_wire_requalifies_in_fresh_scope_before_durable_prepare() {
    let (before, definition) = source();
    let after = rows::append(
        &before,
        definition,
        Profile::DesktopText,
        Input::Text("日本語🦀"),
    );
    let plan = image(&before, 3681)
        .plan_recoverable_retained_omitted_row_inserts(
            "Replacement",
            requests(false),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let wire = literal(
        &before,
        "Replacement",
        vec![vec![None, Some(Value::Text("日本語🦀".into()))]],
    );
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        wire
    );
    let fresh = plan
        .requalify_with_budgets(image(&before, 3682), &mut budget(), &mut budget(), &|| {
            false
        })
        .unwrap();
    assert_eq!(fresh.before_bytes(), before);
    assert_eq!(fresh.after_bytes(), after);
    assert_eq!(fresh.inserted_rows(), 1);

    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(
        fresh
            .prepare(&mut target, &mut journal_file, &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);
    assert!(journal_file.bytes.get_ref().is_empty());
    assert_eq!(target.bytes.get_ref(), &before);
}

#[test]
fn retained_insert_cold_recovery_restores_pending_and_confirms_committed_including_empty_requests()
{
    let (before, definition) = source();
    for empty in [false, true] {
        let after = if empty {
            before.to_vec()
        } else {
            rows::append(
                &before,
                definition,
                Profile::DesktopText,
                Input::Text("日本語🦀"),
            )
        };
        for committed in [false, true] {
            let plan = image(&before, 3681)
                .plan_recoverable_retained_omitted_row_inserts(
                    "Replacement",
                    requests(empty),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            let mut target = MemoryStorage::new(before.to_vec());
            let mut journal_file = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                .unwrap();
            if committed {
                assert_eq!(
                    prepared.commit(&mut budget(), &|| false).unwrap(),
                    usize::from(!empty)
                );
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&after);
            }
            assert!(journal::inspect_v1(&mut journal_file, &mut budget(), &|| false).is_err());
            assert!(journal::inspect_v2(&mut journal_file, &mut budget(), &|| false).is_err());
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let intent =
                NativeOmittedInsertIntent::decode_retained(&wire, &mut budget(), &|| false)
                    .unwrap();
            intent
                .qualify_retained_recovery(checked, CatalogScope::new(3683), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &after } else { &before }
            );

            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            let intent =
                NativeOmittedInsertIntent::decode_retained(&wire, &mut budget(), &|| false)
                    .unwrap();
            intent
                .qualify_retained_recovery(checked, CatalogScope::new(3684), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .confirm_completed(&mut target, &mut budget(), &|| false)
                .unwrap();
        }
    }
}

#[test]
fn retained_recovery_refuses_forged_after_and_old_journal_authority_without_target_writes() {
    let (before, definition) = source();
    let correct = rows::append(
        &before,
        definition,
        Profile::DesktopText,
        Input::Text("日本語🦀"),
    );
    let wire = literal(
        &before,
        "Replacement",
        vec![vec![None, Some(Value::Text("日本語🦀".into()))]],
    );
    let mut forged = correct;
    // An unrelated retired-page byte is outside the requested row action.
    forged[7 * 4096 + 100] ^= 1;
    let mut target = MemoryStorage::new(before.to_vec());
    let mut journal_file = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &before,
            &forged,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    let intent =
        NativeOmittedInsertIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    assert!(
        intent
            .qualify_retained_recovery(checked, CatalogScope::new(3683), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);

    // A typed older request cannot gain V3 authority from a matching BEFORE.
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    let older = NativeOmittedInsertIntent::new_current(
        "Replacement",
        requests(false),
        &before,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert!(
        older
            .qualify_retained_recovery(checked, CatalogScope::new(3683), &mut budget(), &|| false)
            .is_err()
    );
    assert_eq!(target.writes, 0);

    let mut old_journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut old_journal,
            &before,
            &before,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut old_journal, &mut budget(), &|| false).unwrap();
    let intent =
        NativeOmittedInsertIntent::decode_retained(&wire, &mut budget(), &|| false).unwrap();
    assert!(
        qualify_native_omitted_insert_recovery(
            checked,
            intent,
            CatalogScope::new(3683),
            &mut budget(),
            &|| false
        )
        .is_err()
    );
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
