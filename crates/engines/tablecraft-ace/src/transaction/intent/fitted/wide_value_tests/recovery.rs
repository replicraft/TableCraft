//! Fresh native qualification rebuilds exact GUID and Decimal effects under V3.
use super::{budget, groups};
use crate::{
    EditError, NativeImage, NativeRecoveryQualificationError, NativeUpdateIntent,
    RecoverableEditError, WriteBlocker,
};
use std::io::Cursor;
use tablecraft_core::values::{Decimal, Guid, Value};
use tablecraft_io::journal::{self, JournalError};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::{
    fixtures::access::canonical_retained_add_table::{exact_values as raw, fields},
    support::io::journal::MemoryStorage,
};

const TOKEN: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff,
];
const AMOUNT: i128 = 90_071_992_547_409_931_234;

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

fn source() -> (Vec<u8>, u32) {
    let seed = fields::source(fields::Case::FirstNumber).bytes;
    let definition = (seed.len() / 4096) as u32;
    let empty = raw::append_table(&seed, "ExactValues");
    (
        raw::append(&empty, definition, Some(TOKEN), Some(AMOUNT)),
        definition,
    )
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

#[test]
fn guid_and_decimal_originals_rebuild_pending_committed_and_noop_images_in_fresh_scopes() {
    let (before, definition) = source();
    for (column, expected, value) in [
        (
            "Token",
            Value::Guid(Guid::from_bytes(TOKEN)),
            Value::Guid(Guid::from_bytes([0x42; 16])),
        ),
        ("Token", Value::Guid(Guid::from_bytes(TOKEN)), Value::Null),
        (
            "Token",
            Value::Guid(Guid::from_bytes(TOKEN)),
            Value::Guid(Guid::from_bytes(TOKEN)),
        ),
        (
            "Amount",
            Value::Decimal(Decimal::new(AMOUNT, 4).unwrap()),
            Value::Decimal(Decimal::new(-90_071_992_547_409_934_321, 4).unwrap()),
        ),
        (
            "Amount",
            Value::Decimal(Decimal::new(AMOUNT, 4).unwrap()),
            Value::Null,
        ),
        (
            "Amount",
            Value::Decimal(Decimal::new(AMOUNT, 4).unwrap()),
            Value::Decimal(Decimal::new(AMOUNT, 4).unwrap()),
        ),
    ] {
        let after = match &value {
            Value::Guid(value) => raw::update_guid(&before, definition, 0, Some(value.to_bytes())),
            Value::Decimal(value) => {
                raw::update_decimal(&before, definition, 0, Some(value.coefficient()))
            }
            Value::Null if column == "Token" => raw::update_guid(&before, definition, 0, None),
            Value::Null => raw::update_decimal(&before, definition, 0, None),
            _ => unreachable!(),
        };
        for committed in [false, true] {
            let plan = image(&before, 2221)
                .plan_recoverable_retained_wide_value_row_updates(
                    "ExactValues",
                    groups(column, 0, expected.clone(), value.clone()),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_image(plan.after_bytes(), &after);
            let plan = plan
                .requalify_with_budgets(image(&before, 2222), &mut budget(), &mut budget(), &|| {
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
                .qualify_retained_recovery(checked, CatalogScope::new(2223), &mut budget(), &|| {
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
                .qualify_retained_recovery(checked, CatalogScope::new(2224), &mut budget(), &|| {
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
fn stale_expected_wrong_scale_old_protocol_and_forged_after_have_no_target_effects() {
    let (before, definition) = source();
    let expected = Value::Decimal(Decimal::new(AMOUNT, 4).unwrap());
    let value = Value::Decimal(Decimal::new(-90_071_992_547_409_934_321, 4).unwrap());
    let make_plan = || {
        image(&before, 2231)
            .plan_recoverable_retained_wide_value_row_updates(
                "ExactValues",
                groups("Amount", 0, expected.clone(), value.clone()),
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
        image(&before, 2232).plan_recoverable_retained_wide_value_row_updates(
            "ExactValues",
            groups(
                "Token",
                0,
                Value::Null,
                Value::Guid(Guid::from_bytes([0x42; 16]))
            ),
            &mut budget(),
            &|| false,
        ),
        Err(RecoverableEditError::Edit(EditError::Conflict))
    ));
    for invalid in [
        Decimal::new(1, 3).unwrap(),
        Decimal::new(10_000_000_000_000_000_000_000_000_000, 4).unwrap(),
    ] {
        assert!(matches!(
            image(&before, 2233).plan_recoverable_retained_wide_value_row_updates(
                "ExactValues",
                groups("Amount", 0, expected.clone(), Value::Decimal(invalid)),
                &mut budget(),
                &|| false,
            ),
            Err(RecoverableEditError::Edit(EditError::Unsupported(
                WriteBlocker::Column
            )))
        ));
    }
    let mut forged = raw::update_decimal(&before, definition, 0, Some(-90_071_992_547_409_934_321));
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
            .qualify_retained_recovery(checked, CatalogScope::new(2234), &mut budget(), &|| false),
        Err(NativeRecoveryQualificationError::ImageMismatch)
    ));
    assert_eq!(target.writes, 0);
    assert_image(target.bytes.get_ref(), &before);
}
