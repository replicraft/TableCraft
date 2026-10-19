use super::*;
use proptest::prelude::*;
use std::{cell::Cell, collections::BTreeSet};

const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const FIELD_OFFSET: usize = 75;
const FIELD_BYTES: usize = 25;
const MAX_COEFFICIENT: i128 = 9_999_999_999_999_999_999_999_999_999;

type Pair = (Option<i128>, Option<i128>);

fn field_bytes(value: i128) -> Vec<u8> {
    let mut bytes = vec![if value < 0 { 0x80 } else { 0 }];
    for word in value.unsigned_abs().to_be_bytes().as_chunks::<4>().0 {
        bytes.extend(word.iter().rev());
    }
    bytes
}

// Original independent variable-field bytes, not the production Decimal codec.
fn decimal_database(crossed: bool, values: &[Pair], scale: u8) -> Vec<u8> {
    let mut bytes = database(crossed);
    put32(
        &mut bytes,
        TABLE_PAGE * PAGE_BYTES + 16,
        values.len() as u32,
    );
    put16(&mut bytes, TABLE_PAGE * PAGE_BYTES + 43, 2);
    for position in 0..2 {
        let field = TABLE_PAGE * PAGE_BYTES + FIELD_OFFSET + position * FIELD_BYTES;
        bytes[field] = 16;
        bytes[field + 11] = 28;
        bytes[field + 12] = scale;
        bytes[field + 15] = 2;
        put16(&mut bytes, field + 7, position as u16);
        put16(&mut bytes, field + 21, 0);
        put16(&mut bytes, field + 23, 17);
    }
    let rows = values
        .iter()
        .map(|&(a, b)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            let mut offsets = vec![row.len() as u16];
            for value in [a, b] {
                if let Some(value) = value {
                    row.extend(field_bytes(value));
                }
                offsets.push(row.len() as u16);
            }
            for offset in offsets.into_iter().rev() {
                row.extend(offset.to_le_bytes());
            }
            row.extend(2_u16.to_le_bytes());
            row.push(u8::from(a.is_some()) | u8::from(b.is_some()) << 1);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(TABLE_PAGE as u32, &rows));
    bytes
}

#[test]
fn decimal_tuples_preserve_extremes_every_scale_zero_and_null_shapes() {
    let values = [
        (Some(-MAX_COEFFICIENT), Some(MAX_COEFFICIENT)),
        (Some(MAX_COEFFICIENT), Some(-MAX_COEFFICIENT)),
        (Some(9_007_199_254_740_992), Some(9_007_199_254_740_993)),
        (Some(9_007_199_254_740_993), Some(9_007_199_254_740_992)),
        (Some(i64::MAX as i128 + 1), Some(i64::MIN as i128 - 1)),
        (Some(i64::MIN as i128 - 1), Some(i64::MAX as i128 + 1)),
        (Some(0), None),
        (None, Some(0)),
        (None, None),
        (None, None),
    ];
    for code in [1, 2, 5, 6] {
        for scale in 0..=28 {
            for crossed in [false, true] {
                let mut bytes = decimal_database(crossed, &values, scale);
                bytes[20] = code;
                if code == 1 {
                    bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
                }
                assert_eq!(
                    audit(bytes).unwrap().relationships()[0].status(),
                    RelationshipCheckStatus::Checked
                );
            }
        }
    }
}

#[test]
fn decimal_exact_orphans_and_partial_duplicate_parents_fail() {
    for pair in [
        (Some(MAX_COEFFICIENT), Some(MAX_COEFFICIENT - 1)),
        (Some(0), None),
    ] {
        assert_eq!(
            audit(decimal_database(true, &[pair], 28)).err(),
            Some(InspectError::Constraint(ConstraintViolation::MissingParent))
        );
    }
    for pair in [
        (Some(0), Some(0)),
        (None, Some(1)),
        (Some(-MAX_COEFFICIENT), None),
    ] {
        assert_eq!(
            audit(decimal_database(false, &[pair, pair], 4)).err(),
            Some(InspectError::Constraint(
                ConstraintViolation::DuplicateParent
            ))
        );
    }
}

#[test]
fn decimal_pair_parameters_must_match_even_for_empty_or_all_null_tables() {
    for values in [vec![], vec![(None, None)], vec![(Some(0), Some(0))]] {
        for (parameter, value) in [(11, 27), (12, 3)] {
            let mut bytes = decimal_database(true, &values, 4);
            bytes[TABLE_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES + parameter] = value;
            assert_eq!(
                audit(bytes).unwrap().relationships()[0].status(),
                RelationshipCheckStatus::Unsupported
            );
        }
    }
    for code in [1, 2] {
        let mut bytes = decimal_database(false, &[(Some(1), Some(2))], 4);
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        assert_eq!(
            audit(bytes).unwrap().relationships()[0].status(),
            RelationshipCheckStatus::Checked
        );
    }
}

#[test]
fn decimal_tuples_mix_with_integer_currency_and_guid_without_coercion() {
    for (kind, width) in [(0x13, 8), (5, 8), (15, 16)] {
        for crossed in [false, true] {
            let mut bytes =
                decimal_database(crossed, &[(Some(-1), Some(0)), (Some(1), Some(0))], 4);
            put16(&mut bytes, TABLE_PAGE * PAGE_BYTES + 43, 1);
            let field = TABLE_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES;
            bytes[field] = kind;
            bytes[field + 15] = 3;
            put16(&mut bytes, field + 23, width);
            let rows = [-1, 1]
                .into_iter()
                .map(|coefficient| {
                    let mut row = 2_u16.to_le_bytes().to_vec();
                    row.resize(2 + usize::from(width), 0);
                    row.extend(field_bytes(coefficient));
                    let end = row.len() as u16;
                    row.extend(end.to_le_bytes());
                    row.extend((2 + width).to_le_bytes());
                    row.extend(1_u16.to_le_bytes());
                    row.push(3);
                    (0, row)
                })
                .collect::<Vec<_>>();
            bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
                .copy_from_slice(&data_page(TABLE_PAGE as u32, &rows));
            assert_eq!(
                audit(bytes).unwrap().relationships()[0].status(),
                if crossed {
                    RelationshipCheckStatus::Unsupported
                } else {
                    RelationshipCheckStatus::Checked
                }
            );
        }
    }
}

#[test]
fn decimal_relationship_audits_remain_bounded_cancellable_and_read_only() {
    let original = decimal_database(false, &[(Some(MAX_COEFFICIENT), None)], 28);
    let mut input = Cursor::new(original.clone());
    let calls = Cell::new(0);
    let inspect = |source: &mut Cursor<Vec<u8>>,
                   budget: &mut ResourceBudget,
                   cancelled: &dyn Fn() -> bool| {
        check_table_constraints(source, CatalogScope::new(1), "Items", budget, cancelled)
    };
    inspect(
        &mut input,
        &mut ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in [0, calls.get() / 2, calls.get() - 1] {
        let count = Cell::new(0);
        assert!(
            inspect(
                &mut input,
                &mut ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
                &|| {
                    let current = count.get();
                    count.set(current + 1);
                    current >= stop
                }
            )
            .is_err()
        );
    }
    for budget in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(32 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(32 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(inspect(&mut input, &mut { budget }, &|| false).is_err());
    }
    assert_eq!(input.into_inner(), original);
}

proptest! {
    #![proptest_config(ProptestConfig {cases:256,
        rng_seed:proptest::test_runner::RngSeed::Fixed(0x444543464b545550),..Default::default()})]
    #[test]
    fn decimal_native_audits_match_independent_exact_tuple_sets(
        values in prop::collection::vec((prop::option::of(-MAX_COEFFICIENT..=MAX_COEFFICIENT),
            prop::option::of(-MAX_COEFFICIENT..=MAX_COEFFICIENT)),0..12),
        crossed in any::<bool>(), scale in 0_u8..=28,
    ) {
        let parents = values.iter().copied().filter(|pair| *pair != (None,None)).collect::<Vec<_>>();
        let set = parents.iter().copied().collect::<BTreeSet<_>>();
        let expected = if set.len() != parents.len() {
            Some(InspectError::Constraint(ConstraintViolation::DuplicateParent))
        } else if parents.iter().any(|(a,b)| !set.contains(&if crossed {(*b,*a)} else {(*a,*b)})) {
            Some(InspectError::Constraint(ConstraintViolation::MissingParent))
        } else {None};
        let result = audit(decimal_database(crossed,&values,scale));
        if let Some(expected) = expected {prop_assert_eq!(result.err(),Some(expected));}
        else {prop_assert_eq!(result.unwrap().relationships()[0].status(),RelationshipCheckStatus::Checked);}
    }
}
