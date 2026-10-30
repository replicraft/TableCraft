use tablecraft_testkit::access::mutation_fixture as base;
#[path = "composite_indexes/edits.rs"]
mod edits;
#[path = "composite_indexes/failures.rs"]
mod failures;
#[path = "composite_indexes/fixture.rs"]
mod fixture;
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
fn numbers(table: &tablecraft_ace::TableSnapshot) -> Vec<Vec<Option<i64>>> {
    table
        .rows()
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| match value {
                    Value::Null => None,
                    Value::Byte(n) => Some(i64::from(*n)),
                    Value::Int16(n) => Some(i64::from(*n)),
                    Value::Int32(n) => Some(i64::from(*n)),
                    Value::Int64(n) => Some(*n),
                    Value::Currency(n) => Some(n.scaled()),
                    _ => panic!("carrier"),
                })
                .collect()
        })
        .collect()
}

#[test]
fn mixed_widths_directions_component_order_nulls_and_exact_rows_are_checked() {
    let rows = vec![
        vec![
            Some(255),
            Some(-32768),
            Some(i32::MIN as i64),
            Some(i64::MAX),
            Some(-1),
        ],
        vec![
            Some(128),
            Some(32767),
            Some(i32::MAX as i64),
            Some(i64::MIN),
            Some(9_007_199_254_740_993),
        ],
        vec![None, None, None, None, None],
        vec![None, Some(0), None, Some(0), None],
        vec![
            Some(255),
            Some(-32768),
            Some(i32::MIN as i64),
            Some(i64::MAX),
            Some(-1),
        ],
    ];
    let order = [(4, true), (0, false), (3, true), (1, true), (2, false)];
    let mut expected = rows.clone();
    expected.sort_by(|a, b| {
        for &(column, descending) in &order {
            let cmp = a[column].cmp(&b[column]);
            if !cmp.is_eq() {
                return if descending { cmp.reverse() } else { cmp };
            }
        }
        std::cmp::Ordering::Equal
    });
    for code in [5, 6] {
        let bytes = fixture::database(code, &[2, 3, 4, 0x13, 5], &order, &rows, 0);
        assert_eq!(numbers(&read(&bytes).unwrap()), expected);
        assert_eq!(
            check_table(
                &mut Cursor::new(bytes),
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
    }
}

#[test]
fn maximum_width_keys_and_prefixes_cross_every_component_and_row_reference() {
    let kinds = [0x13, 5, 0x13, 5, 0x13, 5, 0x13, 5, 0x13, 5];
    let order = (0..10).map(|i| (i, i % 2 == 0)).collect::<Vec<_>>();
    let row = vec![
        Some(i64::MIN),
        Some(i64::MAX),
        Some(-1),
        Some(0),
        Some(1),
        Some(9_007_199_254_740_992),
        Some(9_007_199_254_740_993),
        Some(i64::MAX),
        Some(-1),
        Some(1),
    ];
    let rows = vec![row; 3];
    for code in [5, 6] {
        assert_eq!(
            numbers(&read(&fixture::database(code, &kinds, &order, &rows, 93)).unwrap()),
            rows
        );
        assert!(
            read(&fixture::database(code, &kinds, &order, &[], 0))
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

#[test]
fn unique_tuples_exempt_only_all_null_and_required_rejects_any_null_component() {
    let order = [(0, false), (1, true)];
    for (rows, flags, expected) in [
        (
            vec![
                vec![None, None],
                vec![None, None],
                vec![None, Some(1)],
                vec![Some(1), None],
            ],
            0x81,
            None,
        ),
        (
            vec![vec![None, Some(1)], vec![None, Some(1)]],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![vec![Some(1), None], vec![Some(1), None]],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![vec![Some(1), Some(1)], vec![Some(1), Some(1)]],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![vec![Some(1), None]],
            0x88,
            Some(IndexConstraintViolation::NullKey),
        ),
        (
            vec![vec![None, Some(1)]],
            0x88,
            Some(IndexConstraintViolation::NullKey),
        ),
        (
            vec![vec![None, None]],
            0x88,
            Some(IndexConstraintViolation::NullKey),
        ),
    ] {
        let mut bytes = fixture::database(6, &[5, 0x13], &order, &rows, 0);
        fixture::index_flags(&mut bytes, flags);
        assert_eq!(
            check_table(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .err(),
            expected.map(InspectError::IndexConstraint)
        );
    }
}

#[test]
fn composite_seeks_unqualified_flags_corruption_and_resources_never_fall_back() {
    let bytes = fixture::database(
        6,
        &[4, 5],
        &[(1, false), (0, true)],
        &[vec![Some(1), Some(1)]],
        0,
    );
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
    for change in 0..5 {
        let mut changed = bytes.clone();
        match change {
            0 => changed[14 * 4096 + 480] = 0x80, // Wrong first component marker.
            1 => changed[14 * 4096 + 489] = 0x7f, // Wrong second component marker.
            2 => changed[14 * 4096 + 488] ^= 1,   // Exact money key disagrees with row.
            3 => fixture::index_flags(&mut changed, 0x82), // Ignore-NULL profile.
            _ => support::put16(&mut changed, 14 * 4096 + 24, 99), // Prefix exceeds tuple bound.
        }
        assert!(read(&changed).is_err());
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

proptest! {
    #![proptest_config(ProptestConfig {cases:256,
        rng_seed:proptest::test_runner::RngSeed::Fixed(0x434f4d50494e4458),..Default::default()})]
    #[test]
    fn compound_order_matches_independent_nullable_tuple_sort(
        values in prop::collection::vec((prop::option::of(any::<i64>()),prop::option::of(any::<i64>())),0..30),
        first_descending in any::<bool>(),second_descending in any::<bool>(),reverse in any::<bool>(),
    ) {
        let mut rows=values.iter().map(|(a,b)|vec![*a,*b]).collect::<Vec<_>>();
        let order=if reverse{[(1,first_descending),(0,second_descending)]}else{[(0,first_descending),(1,second_descending)]};
        let bytes=fixture::database(6,&[5,0x13],&order,&rows,0);
        rows.sort_by(|a,b| {
            for &(column,descending) in &order {let cmp=a[column].cmp(&b[column]);if !cmp.is_eq(){return if descending{cmp.reverse()}else{cmp};}}
            std::cmp::Ordering::Equal
        });
        prop_assert_eq!(numbers(&read(&bytes).unwrap()),rows);
    }
}
