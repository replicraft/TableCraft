#[path = "mutation/fixture.rs"]
mod base;
#[path = "guid_composite_indexes/fixture.rs"]
mod mixed;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;

use proptest::prelude::*;
use std::io::Cursor;
use tablecraft_ace::{
    EditError, IndexCheckStatus, IndexConstraintViolation, InspectError, Int32IndexSelection,
    Int32Update, Int64IndexSelection, NativeImage, WriteBlocker, check_table, read_table_index,
    read_table_index_range, read_table_int64_index_range,
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{LegacyDateTime, Value},
};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn database(descending: bool, values: &[Option<u64>], prefix: usize) -> Vec<u8> {
    let rows = values
        .iter()
        .enumerate()
        .map(|(i, value)| vec![Some(i as i64 + 1), value.map(|bits| bits as i64)])
        .collect::<Vec<_>>();
    numeric_fixture::database(6, &[4, 8], &[(1, descending)], &rows, prefix)
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
                Value::DateTime(value) => Some(value.to_bits()),
                _ => panic!("DATE carrier"),
            };
            (bits, id)
        })
        .collect()
}
fn expected(values: &[Option<u64>], descending: bool) -> Vec<(Option<u64>, i32)> {
    let mut rows = values
        .iter()
        .copied()
        .enumerate()
        .map(|(i, bits)| (bits, i as i32 + 1))
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        let order = match (a.0, b.0) {
            (Some(a), Some(b)) => f64::from_bits(a).total_cmp(&f64::from_bits(b)),
            _ => a.0.cmp(&b.0),
        };
        (if descending { order.reverse() } else { order }).then(a.1.cmp(&b.1))
    });
    rows
}

#[test]
fn date_indexes_preserve_serial_bits_and_physical_order_separately_from_civil_time() {
    let mut values = vec![
        Some(0),
        Some(0x8000000000000000),
        Some(1),
        Some(0x8000000000000001),
        None,
        None,
    ];
    values.extend(
        [
            0.25_f64,
            -0.25,
            -1.25,
            -1.75,
            1.0000000001,
            -1.0000000001,
            45000.123456789,
            -657434.25,
            2958465.9999999995,
        ]
        .map(|x| Some(x.to_bits())),
    );
    for code in [5, 6] {
        for descending in [false, true] {
            let mut bytes = database(descending, &values, 0);
            bytes[20] = code;
            assert_eq!(pairs(&bytes), expected(&values, descending));
            assert_eq!(
                check(&bytes).unwrap().indexes()[0].status(),
                IndexCheckStatus::Checked
            );
            let repeated = [Some((-0.25_f64).to_bits()); 3];
            assert_eq!(
                pairs(&database(descending, &repeated, 12)),
                expected(&repeated, descending)
            );
            assert!(
                read(&database(descending, &[], 0))
                    .unwrap()
                    .rows()
                    .is_empty()
            );
        }
    }
    let negative = LegacyDateTime::from_ole_days(-0.25).unwrap();
    let positive = LegacyDateTime::from_ole_days(0.25).unwrap();
    assert_ne!(negative, positive);
    assert!(negative.temporal_cmp(positive).is_eq());
    let bytes = database(
        false,
        &[Some(negative.to_bits()), Some(positive.to_bits())],
        0,
    );
    assert_eq!(
        &bytes[14 * 4096 + 480..14 * 4096 + 489],
        &[0x7f, 0x40, 0x2f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    );
}

#[test]
fn physical_date_constraints_distinguish_signed_zero_and_calendar_aliases() {
    let alias = vec![
        Some(0),
        Some(0x8000000000000000),
        Some(0.25_f64.to_bits()),
        Some((-0.25_f64).to_bits()),
    ];
    for (flags, values, error) in [
        (0x81, alias.clone(), None),
        (0x89, alias, None),
        (0x81, vec![None, None], None),
        (
            0x81,
            vec![Some(1), Some(1)],
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            0x88,
            vec![None, Some(0)],
            Some(IndexConstraintViolation::NullKey),
        ),
    ] {
        let mut bytes = database(false, &values, 0);
        numeric_fixture::index_flags(&mut bytes, flags);
        assert_eq!(
            check(&bytes).err(),
            error.map(InspectError::IndexConstraint)
        );
    }
}

#[test]
fn date_keys_validate_internal_bounds_as_well_as_returned_rows() {
    let mut bytes = database(false, &[Some(0)], 0);
    bytes.resize(16 * 4096, 0);
    let leaf = bytes[14 * 4096..15 * 4096].to_vec();
    bytes[15 * 4096..16 * 4096].copy_from_slice(&leaf);
    bytes[13 * 4096 + 4095] = 0xc0; // Own both index pages 14 and 15.
    let root = &mut bytes[14 * 4096..15 * 4096];
    root[0] = 3;
    root[27..480].fill(0);
    root[480 + 13..].fill(0);
    root[480 + 13..480 + 17].copy_from_slice(&15_u32.to_be_bytes());
    root[29] = 2;
    support::put16(root, 2, 3599);
    assert_eq!(pairs(&bytes), vec![(Some(0), 1)]);
    for raw in [
        f64::NAN.to_bits(),
        f64::INFINITY.to_bits(),
        (-657435_f64).to_bits(),
        2958466_f64.to_bits(),
    ] {
        let mut changed = bytes.clone();
        let encoded = numeric_fixture::floating_key(8, raw as i64);
        changed[14 * 4096 + 481..14 * 4096 + 489].copy_from_slice(&encoded);
        assert_eq!(read(&changed).err(), Some(InspectError::Malformed));
    }
    for raw in [
        f64::NAN.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        2958466_f64.to_bits(),
    ] {
        assert_eq!(
            read(&database(false, &[Some(raw)], 0)).err(),
            Some(InspectError::Malformed)
        );
    }
}

#[test]
fn date_composites_retain_exact_guid_currency_boolean_and_maximum_prefixes() {
    use mixed::Cell::{Guid, Number};
    let rows = vec![
        vec![
            Some(Number((-0.25_f64).to_bits() as i64)),
            Some(Guid([0; 16])),
            Some(Number(i64::MAX)),
            Some(Number(0)),
        ],
        vec![
            Some(Number(0.25_f64.to_bits() as i64)),
            None,
            Some(Number(i64::MIN)),
            Some(Number(-1)),
        ],
        vec![None, Some(Guid([0xff; 16])), None, Some(Number(0))],
    ];
    let bytes = mixed::database(
        6,
        &[8, 15, 5, 1],
        &[(3, true), (1, false), (0, true), (2, false)],
        &rows,
        0,
    );
    assert_eq!(read(&bytes).unwrap().rows().len(), 3);
    assert!(check(&bytes).is_ok());
    let values = vec![vec![Some((-1.0000000001_f64).to_bits() as i64); 10]; 3];
    let order = (0..10).map(|i| (i, i % 2 == 0)).collect::<Vec<_>>();
    let bytes = numeric_fixture::database(6, &[8; 10], &order, &values, 93);
    assert_eq!(read(&bytes).unwrap().rows().len(), 3);
    assert!(check(&bytes).is_ok());
}

#[test]
fn date_typed_seek_resource_and_write_boundaries_stay_explicit() {
    let mut bytes = database(false, &[Some(0), Some(1)], 0);
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

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x44415445494e4458),..Default::default()})]
    #[test]
    fn arbitrary_valid_date_bits_keep_exact_serial_order(values in prop::collection::vec(prop::option::of(any::<u64>().prop_filter("valid DATE",|bits|LegacyDateTime::from_bits(*bits).is_ok())),0..40),descending in any::<bool>()) {
        prop_assert_eq!(pairs(&database(descending,&values,0)),expected(&values,descending));
    }
}
