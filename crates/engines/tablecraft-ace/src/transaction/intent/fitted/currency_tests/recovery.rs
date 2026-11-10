//! Retained Currency requests reconstruct complete native proof before V3 effects.
use super::{budget, literal_column};
use crate::{
    EditError, FixedScalarUpdate, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent, NativeUpdateProfile,
    WriteBlocker, qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::{Currency, Value};
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{currency as raw, fields},
    support::io::journal::MemoryStorage,
};

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "MoneyTable");
    let first = raw::append(&empty, definition, raw::Input::Omitted);
    (
        raw::append(&first, definition, raw::Input::Value(12_345)),
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

fn groups() -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained(
            "Amount",
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                1,
                Value::Currency(Currency::from_scaled(12_345)),
                Value::Currency(Currency::from_scaled(-20_001)),
            )]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

#[test]
fn currency_original_witnesses_requalify_and_old_prepares_have_no_effects() {
    let (before, definition) = source();
    let after = raw::update(&before, definition, 1, Some(-20_001));
    let plan = image(&before, 1710)
        .plan_recoverable_retained_currency_row_updates(
            "MoneyTable",
            groups(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    let wire = literal_column(
        &before,
        1,
        "MoneyTable",
        "Amount",
        Some(12_345),
        Some(-20_001),
    );
    assert_eq!(
        plan.intent().profile(),
        NativeUpdateProfile::Ace16RetainedCurrencyRows
    );
    assert_eq!(
        plan.intent().encode(&mut budget(), &|| false).unwrap(),
        wire
    );
    assert_eq!(plan.after_bytes(), after);
    let plan = plan
        .requalify_with_budgets(image(&before, 1711), &mut budget(), &mut budget(), &|| {
            false
        })
        .unwrap();
    let mut target = MemoryStorage::new(before.clone());
    let mut journal = MemoryStorage::new(Vec::new());
    assert!(matches!(
        plan.prepare(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    let plan = image(&before, 1712)
        .plan_recoverable_retained_currency_row_updates(
            "MoneyTable",
            groups(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert!(matches!(
        plan.prepare_update(&mut target, &mut journal, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal.writes, 0);
    assert_eq!(target.bytes.get_ref(), &before);
}

#[test]
fn currency_pending_committed_and_noop_recovery_reuse_exact_v3_owner() {
    let (before, definition) = source();
    for noop in [false, true] {
        let value = if noop { 12_345 } else { -20_001 };
        let after = if noop {
            before.clone()
        } else {
            raw::update(&before, definition, 1, Some(value))
        };
        for committed in [false, true] {
            let groups = vec![
                NativeFittedIntentGroup::new_retained(
                    "Amount",
                    NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(
                        1,
                        Value::Currency(Currency::from_scaled(12_345)),
                        Value::Currency(Currency::from_scaled(value)),
                    )]),
                    &mut budget(),
                    &|| false,
                )
                .unwrap(),
            ];
            let plan = image(&before, 1713)
                .plan_recoverable_retained_currency_row_updates(
                    "MoneyTable",
                    groups,
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
                .qualify_retained_recovery(checked, CatalogScope::new(1714), &mut budget(), &|| {
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
                .qualify_retained_recovery(checked, CatalogScope::new(1715), &mut budget(), &|| {
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
fn currency_forged_after_and_old_protocol_authority_refuse_before_target_effects() {
    let (before, definition) = source();
    let wire = literal_column(
        &before,
        1,
        "MoneyTable",
        "Amount",
        Some(12_345),
        Some(-20_001),
    );
    let mut after = raw::update(&before, definition, 1, Some(-20_001));
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
            .qualify_retained_recovery(checked, CatalogScope::new(1716), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
    let mut old_family = wire.clone();
    old_family[..8].copy_from_slice(b"TCNI0017");
    old_family[12..14].copy_from_slice(&17_u16.to_le_bytes());
    let checked = journal::inspect_v3(&mut journal, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&old_family, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(1718), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::Edit(
            EditError::Unsupported(WriteBlocker::Column)
        ))
    ));
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
    assert!(matches!(
        qualify_native_update_recovery(
            checked,
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false).unwrap(),
            CatalogScope::new(1717),
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
