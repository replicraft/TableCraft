//! Fresh opaque INSERT proofs retain original presence and the same V3 owner.
use super::{budget, literal};
use crate::{
    InsertFieldValue, NativeImage, NativeIntentError, NativeOmittedInsertIntent,
    NativeRecoveryQualificationError, OmittedRowInsert, qualify_native_omitted_insert_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{opaque_rows as raw, rows::Input},
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

fn requests(empty: bool) -> Vec<OmittedRowInsert> {
    if empty {
        Vec::new()
    } else {
        vec![OmittedRowInsert::new(vec![
            InsertFieldValue::Omitted,
            InsertFieldValue::Supplied(Value::Null),
        ])]
    }
}

#[test]
fn opaque_insert_requalifies_original_presence_and_refuses_v1_prepare_without_effects() {
    for case in raw::CASES {
        let source = raw::source(case);
        // A completed TCNI18 cell does not erase another row's opaque history.
        let before = raw::after(&source.bytes, source.definition, source.roots[0], Some(123));
        let expected = raw::append(&before, source.definition, Input::Null);
        let plan = image(&before, 1691)
            .plan_recoverable_retained_opaque_omitted_row_inserts(
                "Table1",
                requests(false),
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let wire = literal(&before, vec![vec![None, Some(Value::Null)]]);
        assert_eq!(
            plan.intent().encode(&mut budget(), &|| false).unwrap(),
            wire
        );
        assert_eq!(plan.after_bytes(), expected);
        let plan = plan
            .requalify_with_budgets(image(&before, 1692), &mut budget(), &mut budget(), &|| {
                false
            })
            .unwrap();
        let mut target = MemoryStorage::new(before.clone());
        let mut journal = MemoryStorage::new(Vec::new());
        assert!(matches!(
            plan.prepare(&mut target, &mut journal, &mut budget(), &|| false),
            Err(JournalError::ImageSize)
        ));
        assert_eq!(target.writes, 0);
        assert_eq!(journal.writes, 0);
        assert_eq!(target.bytes.get_ref(), &before);
    }
}

#[test]
fn opaque_insert_pending_committed_and_empty_recovery_keep_original_v3_session() {
    let source = raw::source(raw::Case::VariableRetired);
    for empty in [false, true] {
        let after = if empty {
            source.bytes.clone()
        } else {
            raw::append(&source.bytes, source.definition, Input::Null)
        };
        for committed in [false, true] {
            let plan = image(&source.bytes, 1693)
                .plan_recoverable_retained_opaque_omitted_row_inserts(
                    "Table1",
                    requests(empty),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            let mut target = MemoryStorage::new(source.bytes.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
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
            let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1694), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &after } else { &source.bytes }
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1695), &mut budget(), &|| {
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
fn opaque_insert_forged_after_and_old_protocol_authority_never_write_target() {
    let source = raw::source(raw::Case::VariableRetired);
    let wire = literal(&source.bytes, vec![vec![None, Some(Value::Null)]]);
    let mut after = raw::append(&source.bytes, source.definition, Input::Null);
    after[7 * 4096 + 100] ^= 1;
    let mut target = MemoryStorage::new(source.bytes.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
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
        NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1696), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);

    let mut old_journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_expected(
            &mut target,
            &mut old_journal,
            &source.bytes,
            &source.bytes,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v1(&mut old_journal, &mut budget(), &|| false).unwrap();
    let intent =
        NativeOmittedInsertIntent::decode_retained_opaque(&wire, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        qualify_native_omitted_insert_recovery(
            checked,
            intent,
            CatalogScope::new(1697),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedVersion
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &source.bytes);
}
