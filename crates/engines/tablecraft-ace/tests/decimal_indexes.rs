#[path = "mutation/fixture.rs"]
mod base;
#[path = "decimal_indexes/fixture.rs"]
mod fixture;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;

#[path = "decimal_indexes/selections.rs"]
mod selections;

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
fn pairs(bytes: &[u8], scale: u8) -> Vec<(Option<i128>, i32)> {
    read(bytes)
        .unwrap()
        .rows()
        .iter()
        .map(|row| {
            let Value::Int32(id) = row[0] else {
                panic!("ID")
            };
            let key = match row[1] {
                Value::Null => None,
                Value::Decimal(value) => {
                    assert_eq!(value.scale(), scale);
                    Some(value.coefficient())
                }
                _ => panic!("Decimal carrier"),
            };
            (key, id)
        })
        .collect()
}
fn expected(values: &[Option<i128>], descending: bool) -> Vec<(Option<i128>, i32)> {
    let mut rows = values
        .iter()
        .copied()
        .enumerate()
        .map(|(i, key)| (key, i as i32 + 1))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        let cmp = a.0.cmp(&b.0);
        (if descending { cmp.reverse() } else { cmp }).then(a.1.cmp(&b.1))
    });
    rows
}

#[test]
fn complete_decimal_indexes_preserve_all_28_digits_scale_null_and_prefixes() {
    let max = 10_i128.pow(28) - 1;
    let values = [
        None,
        Some(0),
        Some(1),
        Some(-1),
        Some(9007199254740993),
        Some(9007199254740994),
        Some(9223372036854775808),
        Some(-9223372036854775809),
        Some(311917102708983781990730508),
        Some(-311917102708983781990730508),
        Some(max),
        Some(-max),
        None,
    ];
    for code in [5, 6] {
        for scale in [0, 4, 28] {
            for descending in [false, true] {
                let mut bytes = fixture::database(descending, 28, scale, &values, 0);
                bytes[20] = code;
                assert_eq!(pairs(&bytes, scale), expected(&values, descending));
                assert_eq!(
                    check(&bytes).unwrap().indexes()[0].status(),
                    IndexCheckStatus::Checked
                );
                let repeated = [Some(max); 3];
                assert_eq!(
                    pairs(
                        &fixture::database(descending, 28, scale, &repeated, 21),
                        scale
                    ),
                    expected(&repeated, descending)
                );
                assert!(
                    read(&fixture::database(descending, 28, scale, &[], 0))
                        .unwrap()
                        .rows()
                        .is_empty()
                );
                assert_eq!(
                    pairs(
                        &fixture::database(descending, 28, scale, &[None; 3], 4),
                        scale
                    ),
                    vec![(None, 1), (None, 2), (None, 3)]
                );
            }
        }
    }
    let bytes = fixture::database(false, 28, 4, &[Some(-1), Some(1)], 0);
    assert_eq!(&bytes[14 * 4096 + 480..14 * 4096 + 482], &[0x7f, 0]);
    assert_eq!(
        &bytes[14 * 4096 + 482..14 * 4096 + 498],
        &[
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 254
        ]
    );
    assert_eq!(&bytes[14 * 4096 + 502..14 * 4096 + 504], &[0x7f, 0xff]);
}

#[test]
fn decimal_constraints_remain_exact_beyond_i64_and_floating_precision() {
    for (values, flags, violation) in [
        (
            vec![
                None,
                None,
                Some(9007199254740993),
                Some(9007199254740994),
                Some(9223372036854775808),
            ],
            0x81,
            None,
        ),
        (
            vec![Some(9223372036854775808), Some(9223372036854775808)],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![Some(0), None],
            0x88,
            Some(IndexConstraintViolation::NullKey),
        ),
        (vec![Some(-1), Some(0), Some(1)], 0x89, None),
    ] {
        let mut bytes = fixture::database(false, 28, 4, &values, 0);
        numeric_fixture::index_flags(&mut bytes, flags);
        assert_eq!(
            check(&bytes).err(),
            violation.map(InspectError::IndexConstraint)
        );
    }
}

#[test]
fn decimal_signs_zero_precision_bounds_and_key_agreement_are_strict() {
    let original = fixture::database(false, 4, 2, &[Some(0)], 0);
    for sign in 0..=255 {
        let mut bytes = original.clone();
        bytes[14 * 4096 + 481] = sign;
        if sign == 255 {
            assert!(read(&bytes).is_ok());
        } else {
            assert!(read(&bytes).is_err());
        }
    }
    let mut negative_zero = original.clone();
    negative_zero[14 * 4096 + 481] = 0;
    negative_zero[14 * 4096 + 482..14 * 4096 + 498].fill(255);
    assert_eq!(read(&negative_zero).err(), Some(InspectError::Malformed));
    for value in [10000_i128, -10000] {
        assert_eq!(
            read(&fixture::database(false, 4, 2, &[Some(value)], 0)).err(),
            Some(InspectError::Malformed)
        );
    }
    // Invalid internal upper bound must fail even though its child/row is valid.
    let mut node = original;
    node.resize(16 * 4096, 0);
    let leaf = node[14 * 4096..15 * 4096].to_vec();
    node[15 * 4096..16 * 4096].copy_from_slice(&leaf);
    node[13 * 4096 + 4095] = 0xc0;
    let root = &mut node[14 * 4096..15 * 4096];
    root[0] = 3;
    root[27..480].fill(0);
    root[480 + 22..].fill(0);
    root[502..506].copy_from_slice(&15_u32.to_be_bytes());
    root[30] = 4;
    support::put16(root, 2, 3590);
    assert_eq!(pairs(&node, 2), vec![(Some(0), 1)]);
    node[14 * 4096 + 482..14 * 4096 + 498].copy_from_slice(&10000_u128.to_be_bytes());
    assert_eq!(read(&node).err(), Some(InspectError::Malformed));
    let mut mismatch = fixture::database(false, 28, 4, &[Some(1)], 0);
    let row =
        u16::from_le_bytes(mismatch[12 * 4096 + 14..12 * 4096 + 16].try_into().unwrap()) as usize;
    mismatch[12 * 4096 + row + 19] = 2;
    assert_eq!(read(&mismatch).err(), Some(InspectError::Malformed));
}

#[test]
fn decimal_profile_typed_seek_metadata_and_resource_limits_are_explicit() {
    let original = fixture::database(false, 28, 4, &[Some(1)], 0);
    for selection in [Int32IndexSelection::All, Int32IndexSelection::Null] {
        assert_eq!(
            read_table_index_range(
                &mut Cursor::new(&original),
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
                &mut Cursor::new(&original),
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
    for (offset, value) in [(11, 0), (11, 29), (12, 29), (15, 3), (23, 16)] {
        let mut bytes = original.clone();
        bytes[10 * 4096 + 100 + offset] = value;
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
    let mut compound = original.clone();
    support::put16(&mut compound, 10 * 4096 + 143 + 7, 0);
    compound[10 * 4096 + 143 + 9] = 1;
    // The newly qualified tuple is missing its second encoded component.
    assert_eq!(read(&compound).err(), Some(InspectError::Malformed));
    for flags in [0x82, 0x90] {
        let mut bytes = original.clone();
        numeric_fixture::index_flags(&mut bytes, flags);
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
    for (mut allowance, cancel) in [
        (budget(), true),
        (ResourceBudget::new(66, 100, 10_000), false),
        (ResourceBudget::new(1_000_000, 0, 10_000), false),
        (ResourceBudget::new(1_000_000, 100, 0), false),
    ] {
        assert!(
            read_table_index(
                &mut Cursor::new(&original),
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

#[test]
fn decimal_read_qualification_does_not_enable_ace_edits() {
    let bytes = fixture::database(false, 28, 4, &[Some(1)], 0);
    for column in ["Id", "Score"] {
        let image = NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            image
                .plan_int32_updates(
                    "Items",
                    column,
                    &[Int32Update::new(0, 1, 11)],
                    &mut budget(),
                    &|| false
                )
                .err(),
            Some(EditError::Unsupported(WriteBlocker::Format))
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x444543494e445858),..Default::default()})]
    #[test]
    fn bounded_i128_coefficients_match_independent_exact_integer_order(values in prop::collection::vec(prop::option::of(any::<i128>()),0..40),precision in 1_u8..=28,scale in 0_u8..=28,descending in any::<bool>()) {
        let limit=10_i128.pow(u32::from(precision));let scale=scale.min(precision);
        let values=values.into_iter().map(|v|v.map(|x|x%limit)).collect::<Vec<_>>();
        prop_assert_eq!(pairs(&fixture::database(descending,precision,scale,&values,0),scale),expected(&values,descending));
    }
}

#[test]
fn jet4_decimal_descending_retains_legacy_sign_groups_and_exact_carriers() {
    let values = [None, Some(-10), Some(10), Some(-1), Some(1), Some(0), None];
    for code in [1, 2] {
        for descending in [false, true] {
            let bytes = fixture::database_for(code, descending, 28, 4, &values, 0);
            let actual = pairs(&bytes, 4);
            let expected = if code == 1 && descending {
                vec![
                    (Some(-1), 4),
                    (Some(-10), 2),
                    (Some(10), 3),
                    (Some(1), 5),
                    (Some(0), 6),
                    (None, 1),
                    (None, 7),
                ]
            } else {
                expected(&values, descending)
            };
            assert_eq!(actual, expected);
            assert_eq!(
                check(&bytes).unwrap().indexes()[0].status(),
                IndexCheckStatus::Checked
            );
        }
    }
    // A corrected descending sign byte is not a legacy-compatible retagging.
    let mut retagged = fixture::database(true, 28, 4, &values, 0);
    retagged[4..20].copy_from_slice(b"Standard Jet DB\0");
    retagged[20] = 1;
    assert!(read(&retagged).is_err());
}

#[test]
fn jet4_unrelated_int32_edit_preserves_legacy_decimal_index_bytes() {
    for descending in [false, true] {
        let bytes = fixture::database_for(1, descending, 28, 4, &[Some(-1), Some(1)], 0);
        let image = NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let plan = image
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 1, 11)],
                &mut budget(),
                &|| false,
            )
            .unwrap();
        assert_eq!(plan.changed_rows(), 1);
        assert_eq!(&plan.after_bytes()[14 * 4096..], &bytes[14 * 4096..]);
        assert_eq!(
            check(plan.after_bytes()).unwrap().indexes()[0].status(),
            IndexCheckStatus::Checked
        );
        let actual = pairs(plan.after_bytes(), 4);
        assert_eq!(actual, vec![(Some(-1), 11), (Some(1), 2)]);
    }
}

#[test]
fn native_decimal_shared_schemas_preserve_precision_and_scale_for_all_read_profiles() {
    use tablecraft_ace::{TablePageRequest, inspect_table_schema, read_table, read_table_page};
    use tablecraft_schema::{TypeDescriptor, validate_row};
    for code in [1, 2, 5, 6] {
        let bytes = fixture::database_for(code, false, 8, 3, &[Some(1234), None], 0);
        let inspect = inspect_table_schema(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let expected = TypeDescriptor::decimal(8, 3).unwrap();
        assert_eq!(
            inspect.columns()[1].column().unwrap().type_descriptor(),
            expected
        );
        let sequential = read_table(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let indexed = read(&bytes).unwrap();
        for snapshot in [&sequential, &indexed] {
            assert_eq!(snapshot.columns()[1].type_descriptor(), expected);
            for row in snapshot.rows() {
                validate_row(snapshot.columns(), row).unwrap();
            }
        }
        let page = read_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(page.columns()[1].type_descriptor(), expected);
        let projected = tablecraft_ace::read_table_projection(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &[inspect.columns()[1].name()],
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(projected.columns()[0].type_descriptor(), expected);
        for row in projected.rows() {
            validate_row(projected.columns(), row).unwrap();
        }
    }
}
