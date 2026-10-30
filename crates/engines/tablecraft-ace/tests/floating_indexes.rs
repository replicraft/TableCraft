use tablecraft_testkit::access::mutation_fixture as base;
#[path = "guid_composite_indexes/fixture.rs"]
mod mixed;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, IndexConstraintViolation, InspectError, Int32IndexSelection,
    Int32Update, Int64IndexSelection, NativeImage, WriteBlocker, check_table, read_table_index,
    read_table_index_range, read_table_int64_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn database(kind: u8, descending: bool, values: &[Option<u64>], prefix: usize) -> Vec<u8> {
    let rows = values
        .iter()
        .enumerate()
        .map(|(i, value)| vec![Some(i as i64 + 1), value.map(|bits| bits as i64)])
        .collect::<Vec<_>>();
    numeric_fixture::database(6, &[4, kind], &[(1, descending)], &rows, prefix)
}
fn read(bytes: &[u8]) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| false,
    )
}
fn check(bytes: &[u8]) -> Result<tablecraft_ace::TableCheck, InspectError> {
    check_table(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
}
fn pairs(bytes: &[u8]) -> Vec<(Option<u64>, i32)> {
    read(bytes)
        .unwrap()
        .rows()
        .iter()
        .map(|row| {
            let Value::Int32(id) = row[0] else {
                panic!("ID")
            };
            let bits = match row[1] {
                Value::Null => None,
                Value::Single(value) => Some(u64::from(value.to_bits())),
                Value::Double(value) => Some(value.to_bits()),
                _ => panic!("Exact floating carrier"),
            };
            (bits, id)
        })
        .collect()
}
fn expected(kind: u8, descending: bool, values: &[Option<u64>]) -> Vec<(Option<u64>, i32)> {
    let mut sorted = values
        .iter()
        .copied()
        .enumerate()
        .map(|(i, bits)| (bits, i as i32 + 1))
        .collect::<Vec<_>>();
    sorted.sort_by(|a, b| {
        let cmp = numeric_fixture::compare_cell(kind, a.0.map(|x| x as i64), b.0.map(|x| x as i64));
        (if descending { cmp.reverse() } else { cmp }).then(a.1.cmp(&b.1))
    });
    sorted
}

#[test]
fn ieee_goldens_keep_signed_zero_subnormals_extrema_infinity_and_nan_payloads() {
    for (kind, values) in [
        (
            6,
            vec![
                0, 0x80000000, 1, 0x80000001, 0x007fffff, 0x807fffff, 0x00800000, 0x80800000,
                0x3f800000, 0xbf800000, 0x7f7fffff, 0xff7fffff, 0x7f800000, 0xff800000, 0x7fc00001,
                0xffc00001, 0x7fc00002, 0xffc00002, 0x7f800001, 0xff800001,
            ],
        ),
        (
            7,
            vec![
                0,
                0x8000000000000000,
                1,
                0x8000000000000001,
                0x000fffffffffffff,
                0x800fffffffffffff,
                0x0010000000000000,
                0x8010000000000000,
                0x3ff0000000000000,
                0xbff0000000000000,
                0x7fefffffffffffff,
                0xffefffffffffffff,
                0x7ff0000000000000,
                0xfff0000000000000,
                0x7ff8000000000001,
                0xfff8000000000001,
                0x7ff8000000000002,
                0xfff8000000000002,
                0x7ff0000000000001,
                0xfff0000000000001,
            ],
        ),
    ] {
        let mut values = values.into_iter().map(Some).collect::<Vec<_>>();
        values.extend([None, None]);
        for descending in [false, true] {
            let bytes = database(kind, descending, &values, 0);
            assert_eq!(pairs(&bytes), expected(kind, descending, &values));
            assert_eq!(
                check(&bytes).unwrap().indexes()[0].status(),
                IndexCheckStatus::Checked
            );
            let repeated = vec![values[0]; 3];
            let prefix = if kind == 6 { 8 } else { 12 };
            assert_eq!(
                pairs(&database(kind, descending, &repeated, prefix)),
                expected(kind, descending, &repeated)
            );
            assert!(
                read(&database(kind, descending, &[], 0))
                    .unwrap()
                    .rows()
                    .is_empty()
            );
        }
    }
    // Independent ascending wire goldens for negative zero and positive one.
    let bytes = database(6, false, &[Some(0x80000000), Some(0x3f800000)], 0);
    assert_eq!(
        &bytes[14 * 4096 + 480..14 * 4096 + 485],
        &[0x7f, 0x7f, 0xff, 0xff, 0xff]
    );
    assert_eq!(
        &bytes[14 * 4096 + 489..14 * 4096 + 494],
        &[0x7f, 0xbf, 0x80, 0, 0]
    );
}

#[test]
fn exact_ieee_constraint_keys_distinguish_zeros_and_nan_payloads() {
    for (kind, sign, nan) in [
        (6, 0x80000000, 0x7fc00000),
        (7, 0x8000000000000000, 0x7ff8000000000000),
    ] {
        for (values, flags, violation) in [
            (
                vec![
                    None,
                    None,
                    Some(0),
                    Some(sign),
                    Some(nan + 1),
                    Some(nan + 2),
                ],
                0x81,
                None,
            ),
            (
                vec![Some(nan + 1), Some(nan + 1)],
                0x81,
                Some(IndexConstraintViolation::DuplicateKey),
            ),
            (
                vec![Some(sign), Some(sign)],
                0x81,
                Some(IndexConstraintViolation::DuplicateKey),
            ),
            (
                vec![None, Some(0)],
                0x88,
                Some(IndexConstraintViolation::NullKey),
            ),
            (vec![Some(0), Some(sign)], 0x89, None),
        ] {
            let mut bytes = database(kind, false, &values, 0);
            numeric_fixture::index_flags(&mut bytes, flags);
            assert_eq!(
                check(&bytes).err(),
                violation.map(InspectError::IndexConstraint)
            );
        }
    }
}

#[test]
fn floating_mixed_guid_boolean_currency_and_ten_component_keys_are_exact() {
    use mixed::Cell::{Guid, Number};
    let rows = vec![
        vec![
            Some(Number(0x80000000)),
            Some(Guid([0; 16])),
            Some(Number(i64::MIN)),
            Some(Number(0)),
            Some(Number(i64::MAX)),
        ],
        vec![
            Some(Number(0x7fc00001)),
            None,
            Some(Number(0x7ff8000000000001)),
            Some(Number(-1)),
            None,
        ],
        vec![
            None,
            Some(Guid([0xff; 16])),
            None,
            Some(Number(0)),
            Some(Number(-1)),
        ],
    ];
    let bytes = mixed::database(
        6,
        &[6, 15, 7, 1, 5],
        &[(2, true), (1, false), (3, true), (0, false), (4, false)],
        &rows,
        0,
    );
    assert_eq!(read(&bytes).unwrap().rows().len(), 3);
    assert!(check(&bytes).is_ok());
    let rows = vec![vec![Some(0x8000000000000000_u64 as i64); 10]; 3];
    let order = (0..10).map(|i| (i, i % 2 == 0)).collect::<Vec<_>>();
    let bytes = numeric_fixture::database(6, &[7; 10], &order, &rows, 93);
    assert_eq!(read(&bytes).unwrap().rows().len(), 3);
    assert!(check(&bytes).is_ok());
}

#[test]
fn floating_mismatch_markers_typed_seeks_and_resource_errors_are_explicit() {
    for kind in [6, 7] {
        let bytes = database(kind, true, &[Some(1)], 0);
        for offset in [480, 481, 482] {
            let mut changed = bytes.clone();
            changed[14 * 4096 + offset] ^= 1;
            assert!(read(&changed).is_err());
        }
        for selection in [Int32IndexSelection::All, Int32IndexSelection::Null] {
            assert_eq!(
                read_table_index_range(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "Compound",
                    selection,
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Unsupported)
            );
        }
        for selection in [Int64IndexSelection::All, Int64IndexSelection::Null] {
            assert_eq!(
                read_table_int64_index_range(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "Compound",
                    selection,
                    &mut budget(),
                    &|| false
                )
                .err(),
                Some(InspectError::Unsupported)
            );
        }
        for (mut allowance, cancel) in [
            (budget(), true),
            (ResourceBudget::new(66, 100, 10_000), false),
            (ResourceBudget::new(1_000_000, 0, 10_000), false),
            (ResourceBudget::new(1_000_000, 100, 0), false),
        ] {
            assert!(
                read_table_index(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "Compound",
                    &mut allowance,
                    &|| cancel
                )
                .is_err()
            );
        }
    }
}

#[test]
fn floating_read_support_retains_write_gates_and_unrelated_byte_proofs() {
    for kind in [6, 7] {
        let mut bytes = database(kind, false, &[Some(0), Some(1)], 0);
        let capture = |bytes: &[u8]| {
            NativeImage::capture(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                &mut budget(),
                &|| false,
            )
            .unwrap()
        };
        assert_eq!(
            capture(&bytes)
                .plan_int32_updates(
                    "Items",
                    "Id",
                    &[Int32Update::new(0, 1, 11)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Format))
        );
        bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        bytes[20] = 1;
        assert_eq!(
            capture(&bytes)
                .plan_int32_updates(
                    "Items",
                    "Score",
                    &[Int32Update::new(0, 0, 1)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Column))
        );
        let plan = capture(&bytes)
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 1, 11)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        let row =
            u16::from_le_bytes(bytes[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap()) as usize;
        let field = 12 * 4096 + row + 2..12 * 4096 + row + 6;
        for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
            assert!(before == after || field.contains(&offset));
        }
        assert_eq!(pairs(plan.after_bytes()), vec![(Some(0), 11), (Some(1), 2)]);
    }
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x464c4f4154494e44),..Default::default()})]
    #[test]
    fn all_ieee_bit_patterns_match_std_total_order(values in prop::collection::vec(prop::option::of(any::<u64>()),0..40),single in any::<bool>(),descending in any::<bool>()) {
        let kind=if single {6} else {7};
        let values=values.into_iter().map(|v|v.map(|b|if single {u64::from(b as u32)} else {b})).collect::<Vec<_>>();
        prop_assert_eq!(pairs(&database(kind,descending,&values,0)),expected(kind,descending,&values));
    }
}
