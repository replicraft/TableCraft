#[path = "mutation/fixture.rs"]
mod base;
#[path = "guid_composite_indexes/fixture.rs"]
mod mixed;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;

#[path = "boolean_indexes/selections.rs"]
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
fn database(code: u8, descending: bool, values: &[bool], prefix: usize) -> Vec<u8> {
    let rows = values
        .iter()
        .enumerate()
        .map(|(i, value)| vec![Some(i as i64 + 1), Some(if *value { -1 } else { 0 })])
        .collect::<Vec<_>>();
    numeric_fixture::database(code, &[4, 1], &[(1, descending)], &rows, prefix)
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
fn pairs(bytes: &[u8]) -> Vec<(bool, i32)> {
    read(bytes)
        .unwrap()
        .rows()
        .iter()
        .map(|row| {
            let (Value::Int32(id), Value::Boolean(value)) = (&row[0], &row[1]) else {
                panic!("Boolean domain must survive index traversal")
            };
            (*value, *id)
        })
        .collect()
}

#[test]
fn yes_no_order_and_compressed_row_references_retain_both_values() {
    for code in [5, 6] {
        for descending in [false, true] {
            let bytes = database(code, descending, &[false, true, false, true], 0);
            let expected = if descending {
                vec![(false, 1), (false, 3), (true, 2), (true, 4)]
            } else {
                vec![(true, 2), (true, 4), (false, 1), (false, 3)]
            };
            assert_eq!(pairs(&bytes), expected);
            assert_eq!(
                check(&bytes).unwrap().indexes()[0].status(),
                IndexCheckStatus::Checked
            );
            for value in [false, true] {
                assert_eq!(
                    pairs(&database(code, descending, &[value; 3], 5)),
                    vec![(value, 1), (value, 2), (value, 3)]
                );
            }
            assert!(
                read(&database(code, descending, &[], 0))
                    .unwrap()
                    .rows()
                    .is_empty()
            );
        }
    }
}

#[test]
fn boolean_atoms_have_no_null_and_unique_false_is_not_exempt() {
    for value in [false, true] {
        let mut bytes = database(6, false, &[value, value], 0);
        numeric_fixture::index_flags(&mut bytes, 0x81);
        assert_eq!(
            check(&bytes).err(),
            Some(InspectError::IndexConstraint(
                IndexConstraintViolation::DuplicateKey
            ))
        );
    }
    let mut bytes = database(6, false, &[false, true], 0);
    for flags in [0x81, 0x88, 0x89] {
        numeric_fixture::index_flags(&mut bytes, flags);
        assert!(check(&bytes).is_ok());
    }
    // A false Boolean plus nullable Currency is a partial NULL tuple, not all NULL.
    let rows = vec![vec![Some(0), None], vec![Some(0), None]];
    let mut bytes = numeric_fixture::database(6, &[1, 5], &[(0, false), (1, true)], &rows, 0);
    numeric_fixture::index_flags(&mut bytes, 0x81);
    assert_eq!(
        check(&bytes).err(),
        Some(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        ))
    );
    numeric_fixture::index_flags(&mut bytes, 0x88);
    assert_eq!(
        check(&bytes).err(),
        Some(InspectError::IndexConstraint(
            IndexConstraintViolation::NullKey
        ))
    );
}

#[test]
fn mixed_guid_numeric_and_ten_boolean_components_share_strict_tuple_checks() {
    use mixed::Cell::{Guid, Number};
    let rows = vec![
        vec![Some(Number(0)), Some(Guid([0; 16])), Some(Number(i64::MAX))],
        vec![Some(Number(-1)), Some(Guid([0xff; 16])), None],
        vec![Some(Number(0)), None, Some(Number(-1))],
        vec![Some(Number(-1)), None, Some(Number(i64::MIN))],
    ];
    let order = [(1, true), (0, false), (2, true)];
    let bytes = mixed::database(6, &[1, 15, 5], &order, &rows, 0);
    assert!(check(&bytes).is_ok());
    assert_eq!(read(&bytes).unwrap().rows()[0][0], Value::Boolean(true));
    let ten = vec![vec![Some(-1); 10]; 3];
    let order = (0..10).map(|i| (i, i % 2 == 0)).collect::<Vec<_>>();
    let bytes = numeric_fixture::database(6, &[1; 10], &order, &ten, 23);
    assert_eq!(read(&bytes).unwrap().rows().len(), 3);
    assert!(check(&bytes).is_ok());
}

#[test]
fn invalid_boolean_bytes_null_markers_seeks_and_resource_failures_are_rejected() {
    for descending in [false, true] {
        let original = database(6, descending, &[true], 0);
        for raw in 0..=255_u8 {
            let mut bytes = original.clone();
            bytes[14 * 4096 + 481] = raw;
            let actual = read(&bytes);
            if raw == if descending { 255 } else { 0 } {
                assert!(actual.is_ok());
            } else {
                assert!(actual.is_err(), "invalid or mismatched byte {raw}");
            }
        }
        for marker in [0, 0xff] {
            let mut bytes = original.clone();
            bytes[14 * 4096 + 480] = marker;
            assert!(read(&bytes).is_err());
        }
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
        let mut ignored = original;
        numeric_fixture::index_flags(&mut ignored, 0x82);
        assert_eq!(read(&ignored).err(), Some(InspectError::Unsupported));
    }
}

#[test]
fn boolean_reads_preserve_existing_write_gates_and_unrelated_jet4_bytes() {
    let mut bytes = database(6, false, &[false, true], 0);
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
    assert_eq!(pairs(plan.after_bytes()), vec![(true, 2), (false, 11)]);
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x424f4f4c494e4458),..Default::default()})]
    #[test]
    fn boolean_physical_order_matches_independent_two_value_model(values in prop::collection::vec(any::<bool>(),0..40), descending in any::<bool>()) {
        let mut expected = values.iter().copied().enumerate().map(|(i,value)|(value,i as i32+1)).collect::<Vec<_>>();
        expected.sort_by(|a,b| {let cmp = b.0.cmp(&a.0);(if descending {cmp.reverse()} else {cmp}).then(a.1.cmp(&b.1))});
        prop_assert_eq!(pairs(&database(6,descending,&values,0)),expected);
    }
}
