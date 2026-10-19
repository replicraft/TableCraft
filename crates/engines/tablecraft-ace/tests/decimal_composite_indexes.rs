#[path = "mutation/fixture.rs"]
mod base;
#[path = "decimal_composite_indexes/fixture.rs"]
mod fixture;
#[path = "composite_indexes/fixture.rs"]
mod numeric_fixture;
mod support;

use fixture::Cell;
use proptest::prelude::*;
use std::{cmp::Ordering, io::Cursor};
use tablecraft_ace::{
    EditError, IndexCheckStatus, IndexConstraintViolation, InspectError, Int32IndexSelection,
    Int32Update, Int64IndexSelection, NativeImage, WriteBlocker, check_table, read_table,
    read_table_index, read_table_index_range, read_table_int64_index_range,
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
fn cells(bytes: &[u8], kinds: &[u8], parameters: &[(u8, u8)]) -> Vec<Vec<Option<Cell>>> {
    let table = read(bytes).unwrap();
    assert_eq!(table.columns().len(), kinds.len());
    table
        .rows()
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, value)| {
                    Some(match (kinds[i], value) {
                        (_, Value::Null) => return None,
                        (1, Value::Boolean(v)) => Cell::Number(if *v { -1 } else { 0 }),
                        (2, Value::Byte(v)) => Cell::Number(i64::from(*v)),
                        (3, Value::Int16(v)) => Cell::Number(i64::from(*v)),
                        (4, Value::Int32(v)) => Cell::Number(i64::from(*v)),
                        (5, Value::Currency(v)) => Cell::Number(v.scaled()),
                        (6, Value::Single(v)) => Cell::Number(i64::from(v.to_bits())),
                        (7, Value::Double(v)) => Cell::Number(v.to_bits() as i64),
                        (8, Value::DateTime(v)) => Cell::Number(v.to_bits() as i64),
                        (15, Value::Guid(v)) => Cell::Guid(v.to_bytes()),
                        (16, Value::Decimal(v)) => {
                            assert_eq!(v.scale(), parameters[i].1);
                            Cell::Decimal(v.coefficient())
                        }
                        (0x13, Value::Int64(v)) => Cell::Number(*v),
                        _ => panic!("Exact declared carrier"),
                    })
                })
                .collect()
        })
        .collect()
}
fn sorted(
    kinds: &[u8],
    order: &[(usize, bool)],
    rows: &[Vec<Option<Cell>>],
) -> Vec<Vec<Option<Cell>>> {
    let mut rows = rows.to_vec();
    rows.sort_by(|a, b| {
        for &(column, descending) in order {
            let cmp = match (a[column], b[column]) {
                (Some(Cell::Number(a)), Some(Cell::Number(b))) if kinds[column] == 6 => {
                    f32::from_bits(a as u32).total_cmp(&f32::from_bits(b as u32))
                }
                (Some(Cell::Number(a)), Some(Cell::Number(b)))
                    if matches!(kinds[column], 7 | 8) =>
                {
                    f64::from_bits(a as u64).total_cmp(&f64::from_bits(b as u64))
                }
                _ => a[column].cmp(&b[column]),
            };
            if !cmp.is_eq() {
                return if descending { cmp.reverse() } else { cmp };
            }
        }
        Ordering::Equal
    });
    rows
}

#[test]
fn decimal_tuples_mix_every_other_qualified_numeric_carrier_without_narrowing() {
    use Cell::{Decimal as D, Number as N};
    let kinds = [16, 2, 3, 4, 0x13, 5, 6, 7, 8, 1];
    let max = 10_i128.pow(28) - 1;
    let rows = vec![
        vec![
            Some(D(max)),
            Some(N(255)),
            Some(N(-32768)),
            Some(N(i32::MIN.into())),
            Some(N(i64::MAX)),
            Some(N(i64::MIN)),
            Some(N(0x7fc00001)),
            Some(N(0xfff8000000000001_u64 as i64)),
            Some(N((-1.0000000001_f64).to_bits() as i64)),
            Some(N(0)),
        ],
        vec![
            Some(D(-max)),
            None,
            Some(N(32767)),
            Some(N(i32::MAX.into())),
            Some(N(i64::MIN)),
            Some(N(i64::MAX)),
            Some(N(0x80000000)),
            Some(N(0)),
            Some(N(0)),
            Some(N(-1)),
        ],
        vec![
            None,
            Some(N(0)),
            None,
            Some(N(-1)),
            None,
            Some(N(0)),
            Some(N(1)),
            Some(N(1)),
            None,
            Some(N(0)),
        ],
    ];
    for code in [5, 6] {
        for reverse in [false, true] {
            let mut order = (0..10).map(|i| (i, i % 2 == 0)).collect::<Vec<_>>();
            if reverse {
                order.reverse();
            }
            let bytes = fixture::database(code, &kinds, &order, &rows, 0);
            assert_eq!(
                cells(&bytes, &kinds, &[(28, 4); 10]),
                sorted(&kinds, &order, &rows)
            );
            assert_eq!(
                check(&bytes).unwrap().indexes()[0].status(),
                IndexCheckStatus::Checked
            );
        }
    }
}

#[test]
fn decimal_guid_tuples_keep_each_fields_scale_order_and_maximum_compressed_prefix() {
    use Cell::{Decimal as D, Guid as G, Number as N};
    let kinds = [4, 16, 16, 15];
    let parameters = [(28, 0), (28, 4), (28, 28), (28, 0)];
    let rows = vec![
        vec![
            Some(N(1)),
            Some(D(9007199254740993)),
            Some(D(-1)),
            Some(G([0; 16])),
        ],
        vec![
            Some(N(2)),
            Some(D(9007199254740994)),
            Some(D(1)),
            Some(G([0xff; 16])),
        ],
        vec![Some(N(3)), None, Some(D(0)), None],
        vec![Some(N(4)), Some(D(0)), None, Some(G([0; 16]))],
    ];
    let order = [(3, true), (2, false), (1, true)];
    let bytes = fixture::with_parameters(6, &kinds, &parameters, &order, &rows, 0);
    assert_eq!(
        cells(&bytes, &kinds, &parameters),
        sorted(&kinds, &order, &rows)
    );
    assert!(check(&bytes).is_ok());
    let parameters = [0, 1, 2, 4, 8, 12, 16, 20, 24, 28].map(|scale| (28, scale));
    let repeated = vec![vec![Some(D(-311917102708983781990730508)); 10]; 3];
    let order = (0..10).map(|i| (i, i % 2 == 0)).collect::<Vec<_>>();
    let bytes = fixture::with_parameters(6, &[16; 10], &parameters, &order, &repeated, 183);
    assert_eq!(cells(&bytes, &[16; 10], &parameters), repeated);
    assert!(check(&bytes).is_ok());
}

#[test]
fn exact_decimal_tuple_constraints_distinguish_all_null_partial_null_and_zero() {
    use Cell::{Decimal as D, Guid as G, Number as N};
    let rows = |keys: Vec<[Option<Cell>; 3]>| {
        keys.into_iter()
            .enumerate()
            .map(|(i, key)| {
                let mut row = vec![Some(N(i as i64 + 1))];
                row.extend(key);
                row
            })
            .collect::<Vec<_>>()
    };
    for (flags, keys, violation) in [
        (0x81, vec![[None, None, None]; 2], None),
        (
            0x81,
            vec![[Some(D(0)), None, None]; 2],
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            0x81,
            vec![[None, None, Some(G([0; 16]))]; 2],
            Some(IndexConstraintViolation::DuplicateKey),
        ),
        (
            0x81,
            vec![
                [Some(D(9223372036854775808)), Some(D(1)), None],
                [Some(D(9223372036854775809)), Some(D(1)), None],
            ],
            None,
        ),
        (
            0x89,
            vec![[Some(D(0)), None, Some(G([0; 16]))]],
            Some(IndexConstraintViolation::NullKey),
        ),
        (
            0x88,
            vec![[None, Some(D(0)), None]],
            Some(IndexConstraintViolation::NullKey),
        ),
        (0x89, vec![[Some(D(0)), Some(D(0)), Some(G([0; 16]))]], None),
    ] {
        let mut bytes = fixture::database(
            6,
            &[4, 16, 16, 15],
            &[(2, true), (3, false), (1, false)],
            &rows(keys),
            0,
        );
        numeric_fixture::index_flags(&mut bytes, flags);
        assert_eq!(
            check(&bytes).err(),
            violation.map(InspectError::IndexConstraint)
        );
    }
}

#[test]
fn decimal_tuple_profiles_integer_seeks_and_unsupported_empty_types_stay_strict() {
    use Cell::{Decimal as D, Number as N};
    let rows = vec![vec![Some(D(1)), Some(N(1))]];
    let bytes = fixture::database(6, &[16, 4], &[(0, false), (1, true)], &rows, 0);
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
    let empty = fixture::database(6, &[16, 4], &[(0, false), (1, false)], &[], 0);
    assert!(read(&empty).unwrap().rows().is_empty());
    for code in [1, 2] {
        let mut old = empty.clone();
        old[20] = code;
        if code == 1 {
            old[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        assert!(
            read_table(
                &mut Cursor::new(&old),
                CatalogScope::new(1),
                "Items",
                &mut budget(),
                &|| false
            )
            .is_ok()
        );
        assert_eq!(read(&old).err(), Some(InspectError::Unsupported));
    }
    let mut text = fixture::database(6, &[16, 0x13], &[(0, false), (1, false)], &[], 0);
    text[10 * 4096 + 100] = 10;
    assert_eq!(read(&text).err(), Some(InspectError::Unsupported));
    for flags in [0x82, 0x90] {
        let mut changed = bytes.clone();
        numeric_fixture::index_flags(&mut changed, flags);
        assert_eq!(read(&changed).err(), Some(InspectError::Unsupported));
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
fn corrupt_decimal_components_and_ace_edits_cannot_bypass_validation() {
    use Cell::{Decimal as D, Guid as G, Number as N};
    let rows = vec![vec![Some(N(1)), Some(D(7)), Some(D(1)), Some(G([0; 16]))]];
    let bytes = fixture::with_parameters(
        6,
        &[4, 16, 16, 15],
        &[(28, 0), (28, 4), (4, 2), (28, 0)],
        &[(3, false), (1, true), (2, false)],
        &rows,
        0,
    );
    assert!(read(&bytes).is_ok());
    for at in [489, 499, 500, 518, 534] {
        let mut changed = bytes.clone();
        changed[14 * 4096 + at] ^= 1;
        assert_eq!(read(&changed).err(), Some(InspectError::Malformed));
    }
    let mut overflow = bytes.clone();
    overflow[14 * 4096 + 519..14 * 4096 + 535].copy_from_slice(&10000_u128.to_be_bytes());
    assert_eq!(read(&overflow).err(), Some(InspectError::Malformed));
    let image = NativeImage::capture(
        &mut Cursor::new(bytes),
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
}

proptest! {
    #![proptest_config(ProptestConfig{cases:256,rng_seed:proptest::test_runner::RngSeed::Fixed(0x4445435455504c45),..Default::default()})]
    #[test]
    fn wide_decimal_tuples_match_independent_typed_nullable_order(values in prop::collection::vec((prop::option::of(any::<i128>()),prop::option::of(any::<i128>()),prop::option::of(any::<[u8;16]>()),any::<bool>()),0..30),directions in any::<[bool;4]>(),reverse in any::<bool>()) {
        use Cell::{Decimal as D,Guid as G,Number as N};
        let limit=10_i128.pow(28);
        let rows=values.into_iter().enumerate().map(|(i,(a,b,g,flag))|vec![Some(N(i as i64+1)),a.map(|v|D(v%limit)),b.map(|v|D(v%limit)),g.map(G),Some(N(if flag{-1}else{0}))]).collect::<Vec<_>>();
        let kinds=[4,16,16,15,1];let parameters=[(28,0),(28,4),(28,28),(28,0),(28,0)];
        let mut order=[1,3,2,4].into_iter().zip(directions).collect::<Vec<_>>();if reverse{order.reverse();}
        let bytes=fixture::with_parameters(6,&kinds,&parameters,&order,&rows,0);
        prop_assert_eq!(cells(&bytes,&kinds,&parameters),sorted(&kinds,&order,&rows));
    }
}
