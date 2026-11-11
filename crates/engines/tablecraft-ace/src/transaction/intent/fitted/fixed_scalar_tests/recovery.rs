//! Exact original scalars reconstruct complete images under the same V3 owner.
use super::budget;
use crate::{
    EditError, FixedScalarUpdate, NativeFittedIntentGroup, NativeFittedUpdates, NativeImage,
    NativeIntentError, NativeRecoveryQualificationError, NativeUpdateIntent, RecoverableEditError,
    WriteBlocker, qualify_native_update_recovery,
};
use std::io::Cursor;
use tablecraft_core::values::{Double, LegacyDateTime, Single, Value};
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{
        fields,
        fixed_scalars::{self as raw, Column, Scalar},
    },
    support::io::journal::MemoryStorage,
};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .position(|(actual, expected)| actual != expected),
        None,
        "first differing image byte"
    );
}

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "Measurements");
    (raw::append(&empty, definition), definition)
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

fn groups(column: Column, expected: Value, value: Value) -> Vec<NativeFittedIntentGroup> {
    vec![
        NativeFittedIntentGroup::new_retained_fixed_scalars(
            column.name(),
            NativeFittedUpdates::Fixed(vec![FixedScalarUpdate::new(0, expected, value)]),
            &mut budget(),
            &|| false,
        )
        .unwrap(),
    ]
}

#[test]
fn six_scalar_originals_and_null_noop_recover_pending_committed_and_confirm_fresh_scopes() {
    let (before, definition) = source();
    for (column, expected, replacement, raw_value) in [
        (
            Column::ByteValue,
            Value::Byte(0),
            Value::Byte(255),
            Some(Scalar::Byte(255)),
        ),
        (
            Column::ShortValue,
            Value::Int16(0),
            Value::Int16(-1234),
            Some(Scalar::Int16(-1234)),
        ),
        (
            Column::LargeValue,
            Value::Int64(0),
            Value::Int64(9_007_199_254_740_993),
            Some(Scalar::Int64(9_007_199_254_740_993)),
        ),
        (
            Column::SingleValue,
            Value::Single(Single::from_bits(0)),
            Value::Single(Single::from_bits(0x3fa0_0000)),
            Some(Scalar::SingleBits(0x3fa0_0000)),
        ),
        (
            Column::DoubleValue,
            Value::Double(Double::from_bits(0)),
            Value::Double(Double::from_bits(0x3ff3_c0ca_428c_59dd)),
            Some(Scalar::DoubleBits(0x3ff3_c0ca_428c_59dd)),
        ),
        (
            Column::DateValue,
            Value::Null,
            Value::DateTime(LegacyDateTime::from_bits(0x4004_0000_0000_0000).unwrap()),
            Some(Scalar::DateTimeBits(0x4004_0000_0000_0000)),
        ),
        (Column::DateValue, Value::Null, Value::Null, None),
    ] {
        let after = raw::update(&before, definition, 0, column, raw_value);
        for committed in [false, true] {
            let plan = image(&before, 2121)
                .plan_recoverable_retained_fixed_scalar_row_updates(
                    "Measurements",
                    groups(column, expected.clone(), replacement.clone()),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_image(plan.after_bytes(), &after);
            let plan = plan
                .requalify_with_budgets(image(&before, 2122), &mut budget(), &mut budget(), &|| {
                    false
                })
                .unwrap();
            let wire = plan.intent().encode(&mut budget(), &|| false).unwrap();
            let mut target = MemoryStorage::new(before.clone());
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
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(2123), &mut budget(), &|| {
                    false
                })
                .unwrap()
                .recover_retained(&mut target, &mut budget(), &|| false)
                .unwrap();
            assert_image(
                target.bytes.get_ref(),
                if committed { &after } else { &before },
            );
            let writes = target.writes;
            let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
            NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
                .unwrap()
                .qualify_retained_recovery(checked, CatalogScope::new(2124), &mut budget(), &|| {
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
fn scalar_conflicts_old_protocol_and_forged_replacements_refuse_without_target_writes() {
    let (before, definition) = source();
    let make_plan = || {
        image(&before, 2131)
            .plan_recoverable_retained_fixed_scalar_row_updates(
                "Measurements",
                groups(
                    Column::LargeValue,
                    Value::Int64(0),
                    Value::Int64(9_007_199_254_740_993),
                ),
                &mut budget(),
                &|| false,
            )
            .unwrap()
    };
    let wire = make_plan()
        .intent()
        .encode(&mut budget(), &|| false)
        .unwrap();
    let mut target = MemoryStorage::new(before.clone());
    let mut journal_file = MemoryStorage::new(Vec::new());
    assert!(matches!(
        make_plan().prepare(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert!(matches!(
        make_plan().prepare_update(&mut target, &mut journal_file, &mut budget(), &|| false),
        Err(JournalError::ImageSize)
    ));
    assert_eq!(target.writes, 0);
    assert_eq!(journal_file.writes, 0);
    assert!(matches!(
        image(&before, 2132).plan_recoverable_retained_fixed_scalar_row_updates(
            "Measurements",
            groups(Column::LargeValue, Value::Int64(1), Value::Int64(2)),
            &mut budget(),
            &|| false
        ),
        Err(RecoverableEditError::Edit(EditError::Conflict))
    ));

    let mut forged = raw::update(
        &before,
        definition,
        0,
        Column::LargeValue,
        Some(Scalar::Int64(9_007_199_254_740_993)),
    );
    forged[7 * 4096 + 100] ^= 1;
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
    assert!(matches!(
        NativeUpdateIntent::decode(&wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(2133), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    let mut old_wire = wire.clone();
    old_wire[..8].copy_from_slice(b"TCNI0017");
    old_wire[12..14].copy_from_slice(&17_u16.to_le_bytes());
    let checked = journal::inspect_v3(&mut journal_file, &mut budget(), &|| false).unwrap();
    assert!(matches!(
        NativeUpdateIntent::decode(&old_wire, &mut budget(), &|| false)
            .unwrap()
            .qualify_retained_recovery(checked, CatalogScope::new(2134), &mut budget(), &|| false),
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
            CatalogScope::new(2135),
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
