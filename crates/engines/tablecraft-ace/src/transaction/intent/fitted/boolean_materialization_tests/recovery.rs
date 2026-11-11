//! Absent and stored Boolean bits retain separate original and recovery authority.
use super::{budget, groups, literal};
use crate::{
    EditError, NativeImage, NativeIntentError, NativeRecoveryQualificationError,
    NativeUpdateIntent, NativeUpdateProfile, RecoverableEditError, RecoverableNativeEditPlan,
    WriteBlocker, qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::{
        canonical_retained_add_table::{fields, typed_fields},
        raw_forwarding_chain,
    },
    support::io::journal::MemoryStorage,
};

fn source(ignored_bit: bool) -> (Vec<u8>, u32) {
    let source = fields::source(fields::Case::FirstNumber);
    let before = typed_fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        "Enabled",
        typed_fields::Kind::Boolean,
    );
    (
        typed_fields::boolean_historical(&before, source.definition, 0, ignored_bit),
        source.definition,
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

fn plan(before: &[u8], value: bool) -> RecoverableNativeEditPlan {
    image(before, 2301)
        .plan_recoverable_retained_boolean_materialization(
            "Table1",
            groups("Enabled", 0, false, Value::Boolean(value)),
            &mut budget(),
            &|| false,
        )
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
fn missing_boolean_noop_and_materialization_recover_pending_committed_and_confirm_fresh() {
    for ignored_bit in [false, true] {
        let (before, definition) = source(ignored_bit);
        for value in [false, true] {
            let after = if value {
                typed_fields::boolean_materialized_true(&before, definition, 0)
            } else {
                before.clone()
            };
            let wire = literal(&before, 0, false, value);
            for committed in [false, true] {
                let reviewed = plan(&before, value);
                assert_eq!(
                    reviewed.intent().profile(),
                    NativeUpdateProfile::Ace16RetainedBooleanMaterialization
                );
                assert_eq!(
                    reviewed.intent().encode(&mut budget(), &|| false).unwrap(),
                    wire
                );
                assert_image(reviewed.after_bytes(), &after);
                let reviewed = reviewed
                    .requalify_with_budgets(
                        image(&before, 2302),
                        &mut budget(),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap();
                let mut target = MemoryStorage::new(before.clone());
                let mut journal_file = MemoryStorage::new(Vec::new());
                let prepared = reviewed
                    .prepare_retained(&mut target, &mut journal_file, &mut budget(), &|| false)
                    .unwrap();
                if committed {
                    prepared.commit(&mut budget(), &|| false).unwrap();
                } else {
                    drop(prepared);
                    target.bytes.get_mut().copy_from_slice(&after);
                }
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(2303),
                        &mut budget(),
                        &|| false,
                    )
                    .unwrap()
                    .recover_retained(&mut target, &mut budget(), &|| false)
                    .unwrap();
                assert_image(
                    target.bytes.get_ref(),
                    if committed { &after } else { &before },
                );
                let writes = target.writes;
                let checked =
                    journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
                NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                    .unwrap()
                    .qualify_retained_recovery(
                        checked,
                        CatalogScope::new(2304),
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
fn original_presence_selects_materialization_then_stored_twenty_without_retry() {
    let (before, definition) = source(true);
    assert!(
        image(&before, 2311)
            .select_native_retained_boolean_materialization(
                "Table1",
                "Enabled",
                0,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(matches!(
        image(&before, 2312).plan_recoverable_retained_boolean_row_updates(
            "Table1",
            groups("Enabled", 0, false, Value::Boolean(true)),
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::RowLayout
        )))
    ));
    let after = typed_fields::boolean_materialized_true(&before, definition, 0);
    assert!(
        !image(&after, 2313)
            .select_native_retained_boolean_materialization(
                "Table1",
                "Enabled",
                0,
                &mut budget(),
                &|| false
            )
            .unwrap()
    );
    assert!(matches!(
        image(&after, 2314).plan_recoverable_retained_boolean_materialization(
            "Table1",
            groups("Enabled", 0, true, Value::Boolean(false)),
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::RowLayout
        )))
    ));
    let stored = image(&after, 2315)
        .plan_recoverable_retained_boolean_row_updates(
            "Table1",
            groups("Enabled", 0, true, Value::Boolean(false)),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        stored.intent().profile(),
        NativeUpdateProfile::Ace16RetainedBooleanRows
    );
    let mut record = raw_forwarding_chain::record(&after, definition + 1, 0).to_vec();
    assert_eq!(&record[..2], &[3, 0]);
    assert_eq!(record.len(), 11);
    record[10] &= !4;
    let expected = raw_forwarding_chain::replace_terminal(&after, definition + 1, 0, &record);
    assert_image(stored.after_bytes(), &expected);
}

#[test]
fn wrong_witness_null_old_twenty_forged_after_and_v1_refuse_without_target_writes() {
    let (before, definition) = source(true);
    assert!(matches!(
        image(&before, 2321).plan_recoverable_retained_boolean_materialization(
            "Table1",
            groups("Enabled", 0, true, Value::Boolean(true)),
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Conflict))
    ));
    assert!(matches!(
        image(&before, 2322).plan_recoverable_retained_boolean_materialization(
            "Table1",
            groups("Enabled", 0, false, Value::Null),
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::Column
        )))
    ));
    let mut target = MemoryStorage::new(before.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(matches!(
        plan(&before, true).prepare(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert!(matches!(
        plan(&before, true)
            .prepare_update(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);
    let wire = literal(&before, 0, false, true);
    let after = typed_fields::boolean_materialized_true(&before, definition, 0);
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal_file,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let mut older = wire.clone();
    older[..8].copy_from_slice(b"TCNI0020");
    older[12..14].copy_from_slice(&20_u16.to_le_bytes());
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&older, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(2323), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::RowLayout)
        ))
    ));
    let mut forged = after;
    forged[7 * 4096 + 100] ^= 1;
    let mut forged_journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut forged_journal,
            &before,
            &forged,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut forged_journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(2324), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
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
    assert!(matches!(
        qualify_native_update_recovery(
            checked,
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(2325),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedProfile
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_image(target.bytes.get_ref(), &before);
}
