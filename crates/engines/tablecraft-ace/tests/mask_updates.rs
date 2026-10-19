#![cfg(feature = "ace16-write-preview")]

#[path = "mutation/fixture.rs"]
mod base;
#[path = "mask_updates/fixture.rs"]
mod fixture;
mod support;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{EditError, FixedScalarUpdate, NativeImage, WriteBlocker, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 100_000, 10_000_000)
}
fn capture(bytes: &[u8]) -> NativeImage {
    NativeImage::capture(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        &mut budget(),
        &|| false,
    )
    .unwrap()
}
fn values(bytes: &[u8]) -> Vec<Vec<Value>> {
    read_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
    .rows()
    .to_vec()
}
fn row_start(bytes: &[u8], slot: usize) -> usize {
    12 * 4096
        + (u16::from_le_bytes(
            bytes[12 * 4096 + 14 + slot * 2..12 * 4096 + 16 + slot * 2]
                .try_into()
                .unwrap(),
        ) as usize
            & 0x1fff)
}

#[test]
fn boolean_changes_only_its_own_bit_across_mask_byte_boundaries() {
    for code in [5, 6] {
        for (name, column, bit, old) in [
            ("First", 0, 0, true),
            ("Guard", 6, 6, false),
            ("Seventh", 7, 7, true),
            ("Eighth", 8, 0, false),
            ("Last", 9, 1, true),
        ] {
            let bytes = fixture::database(code, &[false]);
            let preview = capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    name,
                    &[FixedScalarUpdate::new(
                        0,
                        Value::Boolean(old),
                        Value::Boolean(!old),
                    )],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            let mut expected = bytes.clone();
            let at = row_start(&bytes, 0) + 32 + column / 8;
            expected[at] ^= 1 << bit;
            assert_eq!(preview.candidate_bytes(), expected);
            assert_eq!(preview.changed_rows(), 1);
            let mut logical = values(&bytes);
            logical[0][column] = Value::Boolean(!old);
            assert_eq!(values(preview.candidate_bytes()), logical);
            let no_op = capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    name,
                    &[FixedScalarUpdate::new(
                        0,
                        Value::Boolean(old),
                        Value::Boolean(old),
                    )],
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
            assert_eq!(no_op.candidate_bytes(), bytes);
            assert_eq!(no_op.changed_rows(), 0);
        }
    }
}

#[test]
fn null_transitions_retain_ignored_payloads_and_preserve_neighbor_bits() {
    for code in [5, 6] {
        let bytes = fixture::database(code, &[false, true, true]);
        let updates = [
            FixedScalarUpdate::new(0, Value::Int64(7), Value::Null),
            FixedScalarUpdate::new(1, Value::Null, Value::Int64(0)),
            FixedScalarUpdate::new(2, Value::Null, Value::Null),
        ];
        let preview = capture(&bytes)
            .preview_ace16_fixed_updates("Items", "Large", &updates, &mut budget(), &|| false)
            .unwrap();
        let mut expected = bytes.clone();
        expected[row_start(&bytes, 0) + 32] &= !8;
        let second = row_start(&bytes, 1);
        expected[second + 7..second + 15].fill(0);
        expected[second + 32] |= 8;
        assert_eq!(preview.candidate_bytes(), expected);
        assert_eq!(preview.changed_rows(), 2);
        let mut logical = values(&bytes);
        logical[0][3] = Value::Null;
        logical[1][3] = Value::Int64(0);
        assert_eq!(values(preview.candidate_bytes()), logical);
        assert_eq!(
            &preview.candidate_bytes()[row_start(&bytes, 0) + 7..row_start(&bytes, 0) + 15],
            &7_i64.to_le_bytes()
        );
    }
}

#[test]
fn null_is_not_false_zero_or_an_exact_wrong_type() {
    let bytes = fixture::database(6, &[false, true]);
    for (name, row, old, new) in [
        ("First", 0, Value::Null, Value::Boolean(false)),
        ("First", 0, Value::Boolean(true), Value::Null),
        ("Large", 1, Value::Null, Value::Int32(0)),
    ] {
        assert_eq!(
            capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    name,
                    &[FixedScalarUpdate::new(row, old, new)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Column))
        );
    }
    for (row, old, new) in [
        (0, Value::Null, Value::Int64(9)),
        (1, Value::Int64(7), Value::Null),
        (7, Value::Null, Value::Null),
    ] {
        assert_eq!(
            capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Large",
                    &[FixedScalarUpdate::new(row, old, new)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Conflict)
        );
    }
    for (mut allowance, cancelled) in [(budget(), true), (ResourceBudget::new(0, 0, 0), false)] {
        assert!(
            capture(&bytes)
                .preview_ace16_fixed_updates(
                    "Items",
                    "Large",
                    &[FixedScalarUpdate::new(0, Value::Int64(7), Value::Null)],
                    &mut allowance,
                    &|| cancelled
                )
                .is_err()
        );
    }
    let duplicate = [
        FixedScalarUpdate::new(1, Value::Null, Value::Null),
        FixedScalarUpdate::new(1, Value::Null, Value::Int64(9)),
    ];
    assert_eq!(
        capture(&bytes)
            .preview_ace16_fixed_updates("Items", "Large", &duplicate, &mut budget(), &|| false)
            .err(),
        Some(EditError::Conflict)
    );
}

#[test]
fn field_validation_and_late_cancellation_never_publish_a_candidate() {
    let mut restricted = base::with_rule("ValidationRule", "Is Not Null");
    restricted[4..20].copy_from_slice(b"Standard ACE DB\0");
    restricted[20] = 6;
    assert!(
        capture(&restricted)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(
                    1,
                    Value::Int32(20),
                    Value::Int32(27)
                )],
                &mut budget(),
                &|| false
            )
            .is_ok()
    );
    assert_eq!(
        capture(&restricted)
            .preview_ace16_fixed_updates(
                "Items",
                "Score",
                &[FixedScalarUpdate::new(1, Value::Int32(20), Value::Null)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Inspect(
            tablecraft_ace::InspectError::Constraint(
                tablecraft_ace::ConstraintViolation::FieldRule
            )
        ))
    );
    let bytes = fixture::database(6, &[false, true]);
    let calls = std::cell::Cell::new(0_usize);
    let update = [FixedScalarUpdate::new(0, Value::Int64(7), Value::Null)];
    let _ = capture(&bytes)
        .preview_ace16_fixed_updates("Items", "Large", &update, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
    let last = calls.get();
    assert!(last > 1);
    calls.set(0);
    assert_eq!(
        capture(&bytes)
            .preview_ace16_fixed_updates("Items", "Large", &update, &mut budget(), &|| {
                calls.set(calls.get() + 1);
                calls.get() >= last
            })
            .err(),
        Some(EditError::Inspect(tablecraft_ace::InspectError::Cancelled))
    );
    assert_eq!(calls.get(), last);
}

proptest! {
    #![proptest_config(ProptestConfig{cases:128,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4d41534b4e554c4c),..Default::default()})]
    #[test]
    fn multirow_presence_transitions_match_an_independent_model(missing in prop::collection::vec(any::<bool>(),1..32),make_null in any::<bool>(),new_value in any::<i64>()) {
        let bytes=fixture::database(6,&missing);
        let updates=missing.iter().enumerate().map(|(i,m)|FixedScalarUpdate::new(i,if *m {Value::Null}else{Value::Int64(7)},if make_null{Value::Null}else{Value::Int64(new_value)})).collect::<Vec<_>>();
        let preview=capture(&bytes).preview_ace16_fixed_updates("Items","Large",&updates,&mut budget(),&||false).unwrap();
        let mut logical=values(&bytes);let mut changed=0;
        for row in &mut logical {let value=if make_null{Value::Null}else{Value::Int64(new_value)};changed+=usize::from(row[3]!=value);row[3]=value;}
        prop_assert_eq!(preview.changed_rows(),changed);prop_assert_eq!(values(preview.candidate_bytes()),logical);
    }
}
