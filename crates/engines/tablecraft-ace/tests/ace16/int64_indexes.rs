use super::{budget, int64_index_fixture::database};
use std::io::Cursor;
use tablecraft_ace::{
    IndexCheckStatus, IndexConstraintViolation, InspectError, Int32IndexSelection, check_table,
    read_table_index, read_table_index_range,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

#[test]
fn int64_index_order_exact_keys_null_duplicates_and_checked_coverage() {
    let numbers = [
        Some(i64::MAX),
        Some(9_007_199_254_740_993),
        None,
        Some(i64::MIN),
        Some(-9_007_199_254_740_993),
        Some(0),
        Some(-1),
        None,
        Some(i64::MAX),
    ];
    for code in [5, 6] {
        for descending in [false, true] {
            let bytes = database(code, descending, &numbers);
            let table = read_table_index(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "BigKey",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            let mut expected = numbers.to_vec();
            expected.sort();
            if descending {
                expected.reverse();
            }
            assert_eq!(
                table
                    .rows()
                    .iter()
                    .map(|row| match row[1] {
                        Value::Int64(value) => Some(value),
                        Value::Null => None,
                        _ => panic!("carrier"),
                    })
                    .collect::<Vec<_>>(),
                expected
            );
            let report = check_table(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(report.indexes()[0].status(), IndexCheckStatus::Checked);
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
        }
    }
}

#[test]
fn int64_index_constraint_failures_are_exact_and_atomic() {
    for (numbers, flags, violation) in [
        (
            vec![Some(i64::MAX), Some(i64::MAX)],
            0x81,
            IndexConstraintViolation::DuplicateKey,
        ),
        (
            vec![None, Some(i64::MIN)],
            0x88,
            IndexConstraintViolation::NullKey,
        ),
    ] {
        let mut bytes = database(6, false, &numbers);
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
            Some(InspectError::IndexConstraint(violation))
        );
    }
    let mut bytes = database(
        6,
        false,
        &[
            Some(9_007_199_254_740_992),
            Some(9_007_199_254_740_993),
            None,
            None,
        ],
    );
    bytes[10 * 4096 + 189] = 0x81;
    assert!(
        check_table(
            &mut Cursor::new(bytes),
            CatalogScope::new(1),
            "Items",
            &mut budget(),
            &|| false
        )
        .is_ok()
    );
}

#[test]
fn int64_index_corruption_profile_limits_and_cancellation_fail_without_snapshot() {
    let original = database(6, false, &[Some(i64::MAX)]);
    for change in 0..4 {
        let mut bytes = original.clone();
        match change {
            0 => bytes[14 * 4096 + 488] ^= 1, // valid ordered key disagrees with data
            1 => bytes[14 * 4096 + 480] = 0x80, // wrong direction marker
            2 => bytes[14 * 4096 + 491] = 0x0e, // reference outside owned data
            _ => bytes[20] = 2,               // unqualified old profile
        }
        assert!(
            read_table_index(
                &mut Cursor::new(bytes),
                CatalogScope::new(1),
                "Items",
                "BigKey",
                &mut budget(),
                &|| false
            )
            .is_err()
        );
    }
    for (mut allowance, cancelled) in [(budget(), true), (ResourceBudget::new(66, 1, 100), false)] {
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
}
