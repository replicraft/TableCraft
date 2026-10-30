use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "ace16/scalar_fixture.rs"]
mod scalar_fixture;
use tablecraft_testkit::access as support;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, IndexConstraintViolation, InspectError, Int32IndexSelection,
    Int64IndexSelection, check_table, read_table_index, read_table_index_range,
    read_table_int64_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}

#[path = "narrow_indexes/fixture.rs"]
mod narrow_fixture;
use narrow_fixture::database;

fn read(bytes: &[u8]) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "BigKey",
        &mut budget(),
        &|| false,
    )
}
fn pairs(table: &tablecraft_ace::TableSnapshot) -> Vec<(Option<i64>, i32)> {
    table
        .rows()
        .iter()
        .map(|row| {
            let Value::Int32(id) = row[0] else {
                panic!("ID")
            };
            let key = match row[1] {
                Value::Byte(value) => Some(i64::from(value)),
                Value::Int16(value) => Some(i64::from(value)),
                Value::Null => None,
                _ => panic!("carrier"),
            };
            (key, id)
        })
        .collect()
}

#[test]
fn byte_and_short_full_indexes_preserve_unsigned_sign_extremes_nulls_and_compressed_duplicates() {
    for code in [5, 6] {
        for small in [false, true] {
            for descending in [false, true] {
                let values = if small {
                    vec![
                        Some(-32768),
                        Some(-1),
                        Some(0),
                        Some(32767),
                        None,
                        None,
                        Some(-32768),
                    ]
                } else {
                    vec![
                        Some(0),
                        Some(127),
                        Some(128),
                        Some(255),
                        None,
                        None,
                        Some(255),
                    ]
                };
                let bytes = database(code, small, descending, &values, 0);
                let mut expected = values
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(ordinal, value)| (value, ordinal as i32 + 1))
                    .collect::<Vec<_>>();
                expected.sort_by(|a, b| {
                    let order = a.0.cmp(&b.0);
                    (if descending { order.reverse() } else { order }).then(a.1.cmp(&b.1))
                });
                assert_eq!(pairs(&read(&bytes).unwrap()), expected);
                assert_eq!(
                    check_table(
                        &mut Cursor::new(&bytes),
                        CatalogScope::new(1),
                        "Items",
                        &mut budget(),
                        &|| false
                    )
                    .unwrap()
                    .indexes()[0]
                        .status(),
                    IndexCheckStatus::Checked
                );
                let duplicate = database(
                    code,
                    small,
                    descending,
                    &[Some(if small { -32768 } else { 255 }); 3],
                    if small { 6 } else { 5 },
                );
                assert_eq!(read(&duplicate).unwrap().rows().len(), 3);
                for selection in [Int32IndexSelection::All, Int32IndexSelection::Null] {
                    assert_eq!(
                        read_table_index_range(
                            &mut Cursor::new(&bytes),
                            CatalogScope::new(1),
                            "Items",
                            "BigKey",
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
                            "BigKey",
                            selection,
                            &mut budget(),
                            &|| false
                        )
                        .err(),
                        Some(InspectError::Unsupported)
                    );
                }
            }
        }
    }
}

#[test]
fn narrow_index_constraints_corruption_and_resource_failures_publish_no_snapshot() {
    for small in [false, true] {
        let mut duplicate = database(6, small, false, &[Some(1), Some(1)], 0);
        duplicate[10 * 4096 + 189] = 0x81;
        assert_eq!(
            check_table(
                &mut Cursor::new(duplicate),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            ))
        );
        let mut required = database(6, small, false, &[None, Some(0)], 0);
        required[10 * 4096 + 189] = 0x88;
        assert_eq!(
            check_table(
                &mut Cursor::new(required),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            Some(InspectError::IndexConstraint(
                IndexConstraintViolation::NullKey
            ))
        );
        let original = database(6, small, false, &[Some(1)], 0);
        for offset in [
            14 * 4096 + 480,
            14 * 4096 + 481,
            14 * 4096 + if small { 486 } else { 485 },
        ] {
            let mut changed = original.clone();
            changed[offset] ^= 1;
            assert!(read(&changed).is_err());
        }
        for (mut allowance, cancelled) in
            [(budget(), true), (ResourceBudget::new(66, 1, 100), false)]
        {
            assert!(
                read_table_index(
                    &mut Cursor::new(&original),
                    CatalogScope::new(1),
                    "Items",
                    "BigKey",
                    &mut allowance,
                    &|| cancelled
                )
                .is_err()
            );
        }
        assert!(
            read(&database(6, small, false, &[], 0))
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

proptest! {
    #![proptest_config(proptest::test_runner::Config{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4e4152524f574b59),..Default::default()})]
    #[test]
    fn narrow_index_order_matches_independent_signed_and_unsigned_models(values in prop::collection::vec(prop::option::of(any::<i16>()),0..40),small in any::<bool>(),descending in any::<bool>()) {
        let values=values.into_iter().map(|value|value.map(|value|if small{i64::from(value)}else{i64::from(value as u8)})).collect::<Vec<_>>();
        let bytes=database(6,small,descending,&values,0);let mut expected=values.into_iter().enumerate().map(|(ordinal,value)|(value,ordinal as i32+1)).collect::<Vec<_>>();expected.sort_by(|a,b|{let order=a.0.cmp(&b.0);(if descending{order.reverse()}else{order}).then(a.1.cmp(&b.1))});
        prop_assert_eq!(pairs(&read(&bytes).unwrap()),expected);
    }
}
