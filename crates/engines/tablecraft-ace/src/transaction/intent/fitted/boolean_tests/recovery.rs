//! Boolean requests bind actual non-NULL bit values and exact V3 images.
use super::{budget, groups, literal};
use crate::{
    EditError, NativeImage, NativeIntentError, NativeRecoveryQualificationError,
    NativeUpdateIntent, RecoverableEditError, WriteBlocker, qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::Value;
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{boolean as raw, fields},
    support::io::journal::MemoryStorage,
};

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "BooleanValues");
    let first = raw::append(&empty, definition, raw::Input::Omitted);
    (
        raw::append(&first, definition, raw::Input::Value(true)),
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

#[test]
fn boolean_original_bit_witnesses_requalify_and_null_never_becomes_false() {
    let (before, definition) = source();
    let after = raw::update(&before, definition, 0, true);
    let plan = image(&before, 1741)
        .plan_recoverable_retained_boolean_row_updates(
            "BooleanValues",
            groups("Enabled", 0, false, Value::Boolean(true)),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        literal(&before, "BooleanValues", "Enabled", 0, false, true)
    );
    assert_eq!(plan.after_bytes(), after);
    let plan = plan
        .requalify_with_budgets(image(&before, 1742), &mut budget(), &mut budget(), &|| {
            false
        })
        .unwrap();
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    assert!(matches!(
        plan.prepare(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    let plan = image(&before, 1743)
        .plan_recoverable_retained_boolean_row_updates(
            "BooleanValues",
            groups("Enabled", 0, false, Value::Boolean(true)),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(
        plan.prepare_update(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert!(matches!(
        image(&before, 1744).plan_recoverable_retained_boolean_row_updates(
            "BooleanValues",
            groups("Enabled", 0, false, Value::Null),
            &mut budget(),
            &|| false
        ),
        Err(RecoverableEditError::Edit(EditError::Unsupported(
            WriteBlocker::Column
        )))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}

#[test]
fn boolean_v3_pending_committed_and_noop_recovery_keep_the_original_checked_owner() {
    let (before, definition) = source();
    for value in [true, false] {
        let after = if value {
            raw::update(&before, definition, 0, true)
        } else {
            before.clone()
        };
        for committed in [false, true] {
            let plan = image(&before, 1745)
                .plan_recoverable_retained_boolean_row_updates(
                    "BooleanValues",
                    groups("Enabled", 0, false, Value::Boolean(value)),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(plan.after_bytes(), after);
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            let mut target = MemoryStorage::new(before.clone());
            let mut journal = MemoryStorage::new(Vec::new());
            let prepared = plan
                .prepare_retained(&mut target, &mut journal, &mut budget(), &|| false)
                .unwrap();
            if committed {
                prepared.commit(&mut budget(), &|| false).unwrap();
            } else {
                drop(prepared);
                target.bytes.get_mut().copy_from_slice(&after);
            }
            let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1746), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_eq!(
                target.bytes.get_ref(),
                if committed { &after } else { &before }
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(1747), &mut budget(), &|| {
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
fn boolean_recovery_refuses_forged_after_and_older_action_authority_without_writes() {
    let (before, definition) = source();
    let wire = literal(&before, "BooleanValues", "Enabled", 0, false, true);
    let mut after = raw::update(&before, definition, 0, true);
    after[7 * 4096 + 100] ^= 1;
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    drop(
        journal::prepare_resizable_v3_expected(
            &mut target,
            &mut journal,
            &before,
            &after,
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    );
    let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1748), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    let mut old = wire.clone();
    old[..8].copy_from_slice(b"TCNI0019");
    old[12..14].copy_from_slice(&19_u16.to_le_bytes());
    let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&old, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1749), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::Column)
        ))
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
            CatalogScope::new(1750),
            &mut budget(),
            &|| false
        ),
        Err(NativeRecoveryQualificationError::Intent(
            NativeIntentError::UnsupportedProfile
        ))
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}
