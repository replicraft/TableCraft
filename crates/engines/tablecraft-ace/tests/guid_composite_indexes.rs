#[path = "mutation/fixture.rs"]
mod base;
#[path = "guid_composite_indexes/fixture.rs"]
mod fixture;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
use tablecraft_testkit::access as support;

use fixture::Cell;
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
fn cells(table: &tablecraft_ace::TableSnapshot) -> Vec<Vec<Option<Cell>>> {
    table
        .rows()
        .iter()
        .map(|row| {
            row.iter()
                .map(|value| match value {
                    Value::Null => None,
                    Value::Guid(value) => Some(Cell::Guid(value.to_bytes())),
                    Value::Byte(n) => Some(Cell::Number(i64::from(*n))),
                    Value::Int16(n) => Some(Cell::Number(i64::from(*n))),
                    Value::Int32(n) => Some(Cell::Number(i64::from(*n))),
                    Value::Int64(n) => Some(Cell::Number(*n)),
                    Value::Currency(n) => Some(Cell::Number(n.scaled())),
                    _ => panic!("carrier"),
                })
                .collect()
        })
        .collect()
}

#[test]
fn guid_numeric_components_preserve_nulls_domains_directions_and_native_column_order() {
    let rows = vec![
        vec![
            Some(Cell::Number(i64::MIN)),
            Some(Cell::Guid([0; 16])),
            Some(Cell::Number(i64::MAX)),
            Some(Cell::Number(255)),
            Some(Cell::Number(-32768)),
            Some(Cell::Number(i32::MIN as i64)),
        ],
        vec![
            Some(Cell::Number(i64::MAX)),
            Some(Cell::Guid([0xff; 16])),
            Some(Cell::Number(9_007_199_254_740_993)),
            Some(Cell::Number(128)),
            Some(Cell::Number(32767)),
            Some(Cell::Number(i32::MAX as i64)),
        ],
        vec![None, None, None, None, None, None],
        vec![
            None,
            Some(Cell::Guid([0; 16])),
            None,
            Some(Cell::Number(0)),
            None,
            None,
        ],
    ];
    let order = [
        (1, true),
        (4, false),
        (0, true),
        (2, false),
        (5, true),
        (3, false),
    ];
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
        let bytes = fixture::database(code, &[0x13, 15, 5, 2, 3, 4], &order, &rows, 0);
        assert_eq!(cells(&read(&bytes).unwrap()), expected);
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
fn ten_guid_keys_reconstruct_maximum_prefixes_and_keep_empty_metadata_qualified() {
    let order = (0..10)
        .map(|column| (column, column % 2 == 0))
        .collect::<Vec<_>>();
    let row = (0..10)
        .map(|i| Some(Cell::Guid([i as u8; 16])))
        .collect::<Vec<_>>();
    let rows = vec![row; 3];
    for code in [5, 6] {
        assert_eq!(
            cells(&read(&fixture::database(code, &[15; 10], &order, &rows, 193)).unwrap()),
            rows
        );
        assert!(
            read(&fixture::database(code, &[15; 10], &order, &[], 0))
                .unwrap()
                .rows()
                .is_empty()
        );
    }
}

#[test]
fn partial_null_tuples_require_uniqueness_and_primary_required_values() {
    let g = Some(Cell::Guid([0; 16]));
    let n = Some(Cell::Number(0));
    for (rows, flags, expected) in [
        (
            vec![
                vec![g, n],
                vec![g, None],
                vec![None, n],
                vec![None, None],
                vec![None, None],
            ],
            0x81,
            None,
        ),
        (
            vec![vec![g, None], vec![g, None]],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![vec![None, n], vec![None, n]],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![vec![g, n], vec![g, n]],
            0x81,
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            vec![vec![g, None]],
            0x88,
            Some(IndexConstraintViolation::NullKey),
        ),
        (
            vec![vec![None, n]],
            0x88,
            Some(IndexConstraintViolation::NullKey),
        ),
        (vec![vec![g, n]], 0x88, None),
    ] {
        let mut bytes = fixture::database(6, &[15, 0x13], &[(0, false), (1, true)], &rows, 0);
        numeric_fixture::index_flags(&mut bytes, flags);
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
fn mixed_guid_keys_reject_integer_seeks_malformed_segments_and_resource_failures() {
    let bytes = fixture::database(
        6,
        &[15, 5],
        &[(0, true), (1, false)],
        &[vec![Some(Cell::Guid([0x80; 16])), Some(Cell::Number(1))]],
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
    for change in 0..4 {
        let mut changed = bytes.clone();
        match change {
            0 => changed[14 * 4096 + 489] = 0xf6,
            1 => changed[14 * 4096 + 498] = 8,
            2 => changed[14 * 4096 + 497] ^= 1,
            _ => changed[14 * 4096 + 499] = 0x80,
        };
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

#[test]
fn guid_compound_key_mutation_stays_blocked_but_unrelated_jet4_bytes_are_proven() {
    let mut bytes = fixture::database(
        6,
        &[4, 15, 4],
        &[(1, false), (0, true)],
        &[vec![
            Some(Cell::Number(1)),
            Some(Cell::Guid([0; 16])),
            Some(Cell::Number(7)),
        ]],
        0,
    );
    numeric_fixture::index_flags(&mut bytes, 0x81);
    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
    bytes[20] = 1;
    let capture = || {
        NativeImage::capture(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            &mut budget(),
            &|| false,
        )
        .unwrap()
    };
    assert_eq!(
        capture()
            .plan_int32_updates(
                "Items",
                "Id",
                &[Int32Update::new(0, 1, 11)],
                &mut budget(),
                &|| false
            )
            .err(),
        Some(EditError::Unsupported(WriteBlocker::IndexedColumn))
    );
    let plan = capture()
        .plan_int32_updates(
            "Items",
            "K2",
            &[Int32Update::new(0, 7, 17)],
            &mut budget(),
            &|| false,
        )
        .unwrap();
    assert_eq!(
        &bytes[14 * 4096..15 * 4096],
        &plan.after_bytes()[14 * 4096..15 * 4096]
    );
    let row = u16::from_le_bytes([bytes[12 * 4096 + 14], bytes[12 * 4096 + 15]]) as usize;
    let field = 12 * 4096 + row + 22..12 * 4096 + row + 26;
    for (offset, (before, after)) in bytes.iter().zip(plan.after_bytes()).enumerate() {
        assert!(before == after || field.contains(&offset));
    }
    assert_eq!(
        cells(&read(plan.after_bytes()).unwrap()),
        vec![vec![
            Some(Cell::Number(1)),
            Some(Cell::Guid([0; 16])),
            Some(Cell::Number(17))
        ]]
    );
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x47554944434f4d50),..Default::default()})]
    #[test]
    fn nullable_guid_numeric_tuple_order_matches_independent_sets(values in prop::collection::vec((prop::option::of(any::<[u8;16]>()),prop::option::of(any::<i64>()),prop::option::of(any::<[u8;16]>())),0..15),reverse in any::<bool>(),desc in any::<bool>()) {
        let mut rows=values.iter().map(|(a,b,c)|vec![a.map(Cell::Guid),b.map(Cell::Number),c.map(Cell::Guid)]).collect::<Vec<_>>();
        let order=if reverse{[(2,desc),(1,!desc),(0,desc)]}else{[(0,desc),(1,!desc),(2,desc)]};
        let bytes=fixture::database(6,&[15,5,15],&order,&rows,0);
        rows.sort_by(|a,b|{for &(column,descending) in &order{let cmp=a[column].cmp(&b[column]);if !cmp.is_eq(){return if descending{cmp.reverse()}else{cmp};}}std::cmp::Ordering::Equal});
        prop_assert_eq!(cells(&read(&bytes).unwrap()),rows);
    }
}
