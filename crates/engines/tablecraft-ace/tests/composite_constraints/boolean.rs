use super::*;
use proptest::prelude::*;
use std::{cell::Cell, collections::BTreeSet};

const PAGE_BYTES: usize = 4096;
const TABLE_PAGE: usize = 10;
const DATA_PAGE: usize = 12;
const FIELD_OFFSET: usize = 75;
const FIELD_BYTES: usize = 25;
const PHYSICAL_INDEX: usize = 143;
const KEY_COLUMN_OFFSET: usize = 4;
const KEY_COLUMN_BYTES: usize = 3;
const ROW_COUNT_OFFSET: usize = 16;
const VARIABLE_COUNT_OFFSET: usize = 43;
const FIELD_VARIABLE_NUMBER_OFFSET: usize = 7;
const FIELD_PRECISION_OFFSET: usize = 11;
const FIELD_SCALE_OFFSET: usize = 12;
const FIELD_FLAGS_OFFSET: usize = 15;
const FIELD_FIXED_OFFSET: usize = 21;
const FIELD_LENGTH_OFFSET: usize = 23;
const INDEX_FLAGS_OFFSET: usize = 46;

fn boolean_database(crossed: bool, values: &[(bool, bool)]) -> Vec<u8> {
    let mut bytes = database(crossed);
    put32(
        &mut bytes,
        TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET,
        values.len() as u32,
    );
    for position in 0..2 {
        let field = TABLE_PAGE * PAGE_BYTES + FIELD_OFFSET + position * FIELD_BYTES;
        bytes[field] = 1;
        put16(&mut bytes, field + FIELD_FIXED_OFFSET, 0);
        put16(&mut bytes, field + FIELD_LENGTH_OFFSET, 1);
    }
    let rows = values
        .iter()
        .map(|&(left, right)| {
            // Native Boolean values occupy their own mask bits, not fixed
            // integer bytes. A cleared bit is stored false, never NULL.
            (0, vec![2, 0, u8::from(left) | u8::from(right) << 1])
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(TABLE_PAGE as u32, &rows));
    bytes
}

fn single_boolean_database(values: &[(bool, bool)], one_to_one: bool) -> Vec<u8> {
    let mut bytes = boolean_database(true, values);
    let mut declaration = row(0, true);
    declaration[2..6].copy_from_slice(&i32::from(one_to_one).to_le_bytes());
    declaration[6..10].copy_from_slice(&1_i32.to_le_bytes());
    put32(&mut bytes, 7 * PAGE_BYTES + ROW_COUNT_OFFSET, 1);
    bytes[9 * PAGE_BYTES..10 * PAGE_BYTES].copy_from_slice(&data_page(7, &[(0, declaration)]));
    let second_key =
        TABLE_PAGE * PAGE_BYTES + PHYSICAL_INDEX + KEY_COLUMN_OFFSET + KEY_COLUMN_BYTES;
    put16(&mut bytes, second_key, u16::MAX);
    bytes[second_key + 2] = 0;
    bytes
}

#[test]
fn boolean_membership_checks_both_values_and_false_is_never_null_exempt() {
    for code in [1, 2, 5, 6] {
        let mut bytes = single_boolean_database(&[(false, true), (true, false)], false);
        bytes[20] = code;
        if code == 1 {
            bytes[4..20].copy_from_slice(b"Standard Jet DB\0");
        }
        assert_eq!(
            audit(bytes).unwrap().relationships()[0].status(),
            RelationshipCheckStatus::Checked
        );
    }
    assert_eq!(
        audit(single_boolean_database(&[(true, false)], false)).err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
    assert_eq!(
        audit(single_boolean_database(&[(false, true)], false)).err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
    assert_eq!(
        audit(boolean_database(false, &[(false, false), (false, false)])).err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}

#[test]
fn boolean_parent_and_one_to_one_child_uniqueness_are_separate() {
    assert_eq!(
        audit(single_boolean_database(
            &[(false, false), (false, true)],
            false
        ))
        .err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
    let repeated_child = [(false, false), (true, false)];
    assert_eq!(
        audit(single_boolean_database(&repeated_child, false))
            .unwrap()
            .relationships()[0]
            .status(),
        RelationshipCheckStatus::Checked
    );
    assert_eq!(
        audit(single_boolean_database(&repeated_child, true)).err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateChild
        ))
    );
}

#[test]
fn boolean_composite_keys_match_tuples_not_individual_components() {
    assert_eq!(
        audit(boolean_database(true, &[(false, false), (true, true)]))
            .unwrap()
            .relationships()[0]
            .status(),
        RelationshipCheckStatus::Checked
    );
    assert_eq!(
        audit(boolean_database(true, &[(false, true)])).err(),
        Some(InspectError::Constraint(ConstraintViolation::MissingParent))
    );
    assert_eq!(
        audit(boolean_database(true, &[(false, true), (true, false)]))
            .unwrap()
            .relationships()[0]
            .status(),
        RelationshipCheckStatus::Checked
    );
}

fn boolean_integer_database(crossed: bool, values: &[(bool, Option<i32>)]) -> Vec<u8> {
    let mut bytes = boolean_database(crossed, &[]);
    put32(
        &mut bytes,
        TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET,
        values.len() as u32,
    );
    let integer_field = TABLE_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES;
    bytes[integer_field] = 4;
    put16(&mut bytes, integer_field + FIELD_LENGTH_OFFSET, 4);
    let rows = values
        .iter()
        .map(|&(flag, number)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(number.unwrap_or(0).to_le_bytes());
            row.push(u8::from(flag) | u8::from(number.is_some()) << 1);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(TABLE_PAGE as u32, &rows));
    bytes
}

#[test]
fn mixed_boolean_integer_keys_keep_null_components_and_strict_paired_types() {
    let values = [
        (false, None),
        (true, None),
        (false, Some(0)),
        (true, Some(-1)),
    ];
    assert_eq!(
        audit(boolean_integer_database(false, &values))
            .unwrap()
            .relationships()[0]
            .status(),
        RelationshipCheckStatus::Checked
    );
    assert_eq!(
        audit(boolean_integer_database(true, &values))
            .unwrap()
            .relationships()[0]
            .status(),
        RelationshipCheckStatus::Unsupported
    );
    assert_eq!(
        audit(boolean_integer_database(
            false,
            &[(false, None), (false, None)]
        ))
        .err(),
        Some(InspectError::Constraint(
            ConstraintViolation::DuplicateParent
        ))
    );
}

#[test]
fn boolean_components_survive_the_wider_decimal_key_path() {
    let mut bytes = boolean_database(false, &[]);
    put32(&mut bytes, TABLE_PAGE * PAGE_BYTES + ROW_COUNT_OFFSET, 2);
    put16(
        &mut bytes,
        TABLE_PAGE * PAGE_BYTES + VARIABLE_COUNT_OFFSET,
        1,
    );
    let decimal = TABLE_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES;
    bytes[decimal] = 16;
    put16(&mut bytes, decimal + FIELD_VARIABLE_NUMBER_OFFSET, 0);
    bytes[decimal + FIELD_PRECISION_OFFSET] = 28;
    bytes[decimal + FIELD_SCALE_OFFSET] = 4;
    bytes[decimal + FIELD_FLAGS_OFFSET] = 2;
    put16(&mut bytes, decimal + FIELD_LENGTH_OFFSET, 17);
    // Two equal Decimal zeros remain distinct through their Boolean component.
    // The last byte contains Boolean false/true and non-NULL Decimal bits.
    let rows = [false, true]
        .into_iter()
        .map(|flag| {
            let mut row = vec![2, 0];
            row.extend([0; 17]);
            row.extend([19, 0, 2, 0, 1, 0, 2 | u8::from(flag)]);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(TABLE_PAGE as u32, &rows));
    assert_eq!(
        audit(bytes).unwrap().relationships()[0].status(),
        RelationshipCheckStatus::Checked
    );
}

#[test]
fn boolean_audits_require_the_ordered_parent_unique_index() {
    let original = boolean_database(false, &[(false, true), (true, false)]);
    let mut nonunique = original.clone();
    nonunique[TABLE_PAGE * PAGE_BYTES + PHYSICAL_INDEX + INDEX_FLAGS_OFFSET] = 0;
    assert_eq!(
        audit(nonunique).unwrap().relationships()[0].status(),
        RelationshipCheckStatus::Unsupported
    );
    let mut reversed = original;
    let first = TABLE_PAGE * PAGE_BYTES + PHYSICAL_INDEX + KEY_COLUMN_OFFSET;
    put16(&mut reversed, first, 1);
    put16(&mut reversed, first + KEY_COLUMN_BYTES, 0);
    assert_eq!(
        audit(reversed).unwrap().relationships()[0].status(),
        RelationshipCheckStatus::Unsupported
    );
}

#[test]
fn boolean_audits_preserve_cancellation_and_independent_resource_limits() {
    let bytes = boolean_integer_database(false, &[(false, None), (true, Some(7))]);
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    check_table_constraints(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
        &count,
    )
    .unwrap();
    let total = calls.get();
    assert!(total > 20);

    for stop in [total / 2, total - 2] {
        calls.set(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        };
        let result = check_table_constraints(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000),
            &cancel,
        );
        assert_eq!(result.err(), Some(InspectError::Cancelled));
        assert_eq!(calls.get(), stop);
    }

    for (allocation, rows, steps) in [
        (0, 16_384, 1_000_000),
        (32 * 1024 * 1024, 0, 1_000_000),
        (32 * 1024 * 1024, 16_384, 0),
    ] {
        let result = check_table_constraints(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            &mut ResourceBudget::new(allocation, rows, steps),
            &|| false,
        );
        assert!(matches!(result, Err(InspectError::Limit(_))));
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x424f4f4c464b),
        ..Default::default()
    })]

    #[test]
    fn boolean_tuple_audits_match_an_independent_set_model(
        values in prop::collection::vec((any::<bool>(), any::<bool>()), 0..20),
        crossed in any::<bool>()
    ) {
        let parents: BTreeSet<_> = values.iter().copied().collect();
        let expected = if parents.len() != values.len() {
            Some(InspectError::Constraint(ConstraintViolation::DuplicateParent))
        } else if crossed && values.iter().any(|&(left, right)| !parents.contains(&(right, left))) {
            Some(InspectError::Constraint(ConstraintViolation::MissingParent))
        } else {
            None
        };
        prop_assert_eq!(audit(boolean_database(crossed, &values)).err(), expected);
    }
}
