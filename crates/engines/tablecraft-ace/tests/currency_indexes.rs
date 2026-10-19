#[path = "currency_indexes/fixture.rs"]
mod currency_fixture;
#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "ace16/int64_index_fixture.rs"]
mod int64_index_fixture;
#[path = "ace16/scalar_fixture.rs"]
mod scalar_fixture;
mod support;

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
                Value::Currency(value) => Some(value.scaled()),
                Value::Null => None,
                _ => panic!("Currency carrier"),
            };
            (key, id)
        })
        .collect()
}

#[test]
fn currency_indexes_keep_scaled_extremes_fractional_units_and_compressed_duplicates() {
    let values = [
        Some(i64::MIN),
        Some(i64::MAX),
        Some(-1),
        Some(0),
        Some(1),
        Some(9_007_199_254_740_992),
        Some(9_007_199_254_740_993),
        None,
        None,
    ];
    for code in [5, 6] {
        for descending in [false, true] {
            let bytes = currency_fixture::database(code, descending, &values, 0);
            let mut expected = values
                .iter()
                .copied()
                .enumerate()
                .map(|(i, key)| (key, i as i32 + 1))
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
            let compressed = currency_fixture::database(code, descending, &[Some(i64::MIN); 3], 12);
            assert_eq!(
                pairs(&read(&compressed).unwrap()),
                vec![
                    (Some(i64::MIN), 1),
                    (Some(i64::MIN), 2),
                    (Some(i64::MIN), 3)
                ]
            );
        }
    }
}

#[test]
fn integer_range_apis_reject_currency_even_for_all_and_null() {
    let bytes = currency_fixture::database(6, false, &[None, Some(1)], 0);
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

#[test]
fn currency_constraint_checks_preserve_exact_uniqueness_and_null_rules() {
    for (values, flags, expected) in [
        (
            vec![Some(i64::MAX), Some(i64::MAX)],
            0x81,
            Some(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey,
            )),
        ),
        (
            vec![None, Some(0)],
            0x88,
            Some(InspectError::IndexConstraint(
                IndexConstraintViolation::NullKey,
            )),
        ),
        (
            vec![
                Some(9_007_199_254_740_992),
                Some(9_007_199_254_740_993),
                None,
                None,
            ],
            0x81,
            None,
        ),
    ] {
        let mut bytes = currency_fixture::database(6, false, &values, 0);
        bytes[10 * 4096 + 189] = flags;
        assert_eq!(
            check_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            expected
        );
    }
}

#[test]
fn currency_corruption_empty_sources_and_shared_resource_failures_are_explicit() {
    assert!(
        read(&currency_fixture::database(6, false, &[], 0))
            .unwrap()
            .rows()
            .is_empty()
    );
    let original = currency_fixture::database(6, false, &[Some(i64::MAX)], 0);
    for change in 0..4 {
        let mut bytes = original.clone();
        match change {
            0 => bytes[14 * 4096 + 488] ^= 1, // Decoded row disagrees with the key.
            1 => bytes[14 * 4096 + 480] = 0x80, // Wrong direction marker.
            2 => bytes[14 * 4096 + 491] = 0x0e, // Reference outside owned data.
            _ => bytes[10 * 4096 + 75 + 25] = 10, // Unqualified Text key/layout.
        }
        assert!(read(&bytes).is_err());
    }
    for (mut allowance, cancel) in [
        (budget(), true),
        (ResourceBudget::new(66, 100, 1000), false),
        (ResourceBudget::new(1_000_000, 0, 10_000), false),
        (ResourceBudget::new(1_000_000, 100, 0), false),
    ] {
        assert!(
            read_table_index(
                &mut Cursor::new(&original),
                CatalogScope::new(1),
                "Items",
                "BigKey",
                &mut allowance,
                &|| cancel
            )
            .is_err()
        );
    }
}

#[test]
fn read_coverage_retains_ace_write_gates_and_preserves_unaffected_jet4_money_indexes() {
    let mut bytes = currency_fixture::database(6, false, &[Some(i64::MIN), Some(i64::MAX)], 0);
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
                "Id",
                &[Int32Update::new(0, 1, 11)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Format))
    );

    // Synthetic shared-layout Jet 4 regression, not an Access save/reopen claim.
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
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
                "Score",
                &[Int32Update::new(0, 1, 2)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::Column))
    );
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
    assert_eq!(
        &plan.after_bytes()[14 * 4096..15 * 4096],
        &bytes[14 * 4096..15 * 4096]
    );
    let at = 12 * 4096;
    let row = usize::from(u16::from_le_bytes([bytes[at + 14], bytes[at + 15]]) & 0x1fff);
    let allowed = at + row + 2..at + row + 6;
    for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
        assert!(before == after || allowed.contains(&offset));
    }
    assert_eq!(
        pairs(&read(plan.after_bytes()).unwrap()),
        vec![(Some(i64::MIN), 11), (Some(i64::MAX), 2)]
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x43555252494e4458),
        ..Default::default() })]
    #[test]
    fn arbitrary_currency_keys_match_an_independent_exact_order_model(
        values in prop::collection::vec(prop::option::of(any::<i64>()), 0..50),
        descending in any::<bool>(), code in prop_oneof![Just(5_u8), Just(6_u8)],
    ) {
        let bytes = currency_fixture::database(code, descending, &values, 0);
        let mut expected = values.iter().copied().enumerate()
            .map(|(i, key)| (key, i as i32 + 1)).collect::<Vec<_>>();
        expected.sort_by(|a, b| {
            let order = a.0.cmp(&b.0);
            (if descending { order.reverse() } else { order }).then(a.1.cmp(&b.1))
        });
        prop_assert_eq!(pairs(&read(&bytes).unwrap()), expected);
    }
}
