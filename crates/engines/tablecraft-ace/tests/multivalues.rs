#[path = "mutation/fixture.rs"]
#[allow(dead_code)]
mod base;
#[path = "complex_schema/fixture.rs"]
mod complex_fixture;
#[path = "complex_links/fixture.rs"]
mod links_fixture;
mod support;

use proptest::prelude::*;
use std::{cell::Cell, io::Cursor};
use support::{data_page, put16, put32};
use tablecraft_ace::{
    InspectError, MultivalueColumn, read_complex_column_links, read_multivalue_column,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

const PAGE_BYTES: usize = 4096;
const TEMPLATE_PAGE: usize = 17;
const FLAT_PAGE: usize = 20;
const CHILD_PAGE: usize = 22;
const FIELD_OFFSET: usize = 63;
const FIELD_BYTES: usize = 25;

fn budget() -> ResourceBudget {
    ResourceBudget::new(64 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8]) -> Result<MultivalueColumn, InspectError> {
    read_multivalue_column(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| false,
    )
}
fn rows(bytes: &mut [u8], values: &[Option<i32>]) {
    put32(bytes, FLAT_PAGE * PAGE_BYTES + 16, values.len() as u32);
    let records = values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(10_i32.to_le_bytes());
            row.extend(value.unwrap_or(0).to_le_bytes());
            row.extend((i as i32 + 1).to_le_bytes());
            row.push(if value.is_some() { 7 } else { 5 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &records));
}

#[test]
fn typed_collections_preserve_duplicates_null_cells_empty_and_absent_parents() {
    for code in [2, 5, 6] {
        let mut bytes = links_fixture::populated();
        bytes[20] = code;
        rows(
            &mut bytes,
            &[Some(i32::MIN), Some(i32::MIN), None, Some(i32::MAX)],
        );
        let collection = read(&bytes).unwrap();
        assert_eq!(collection.value_column().data_type(), ScalarType::Int32);
        assert_eq!(
            collection.values(0).unwrap().cloned().collect::<Vec<_>>(),
            [
                Value::Int32(i32::MIN),
                Value::Int32(i32::MIN),
                Value::Null,
                Value::Int32(i32::MAX)
            ]
        );
        assert_eq!(collection.values(1).unwrap().len(), 0);
        assert_eq!(collection.values(2).unwrap().len(), 0);
        assert!(collection.values(3).is_none());
        assert_eq!(collection.links().child_snapshot().rows().len(), 4);
    }
}

#[test]
fn hundred_values_are_admitted_but_overflow_declines_without_limiting_raw_links() {
    let mut bytes = links_fixture::populated();
    rows(&mut bytes, &(0..100).map(Some).collect::<Vec<_>>());
    assert_eq!(read(&bytes).unwrap().values(0).unwrap().len(), 100);
    rows(&mut bytes, &(0..101).map(Some).collect::<Vec<_>>());
    assert_eq!(read(&bytes).err(), Some(InspectError::StructureLimit));
    assert_eq!(
        read_complex_column_links(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "Score",
            &mut budget(),
            &|| false
        )
        .unwrap()
        .parents()[0]
            .child_count(),
        101
    );
}

#[test]
fn exact_large_number_multivalues_keep_their_source_carrier() {
    let mut bytes = links_fixture::populated();
    for at in [
        TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET,
        FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES,
    ] {
        bytes[at] = 0x13;
        put16(&mut bytes, at + 23, 8);
    }
    put16(
        &mut bytes,
        FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + 2 * FIELD_BYTES + 21,
        12,
    );
    let records = [i64::MIN, 9_007_199_254_740_993, i64::MAX]
        .into_iter()
        .enumerate()
        .map(|(i, value)| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(10_i32.to_le_bytes());
            row.extend(value.to_le_bytes());
            row.extend((i as i32 + 1).to_le_bytes());
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_PAGE * PAGE_BYTES..(CHILD_PAGE + 1) * PAGE_BYTES]
        .copy_from_slice(&data_page(FLAT_PAGE as u32, &records));
    assert_eq!(
        read(&bytes)
            .unwrap()
            .values(0)
            .unwrap()
            .cloned()
            .collect::<Vec<_>>(),
        [
            Value::Int64(i64::MIN),
            Value::Int64(9_007_199_254_740_993),
            Value::Int64(i64::MAX)
        ]
    );
}

#[test]
fn conceptual_decimal_hint_is_empty_precise_and_never_a_record_width() {
    let mut bytes = links_fixture::populated();
    rows(&mut bytes, &[]);
    for (page, position, width) in [(TEMPLATE_PAGE, 0, 9), (FLAT_PAGE, 1, 17)] {
        let at = page * PAGE_BYTES + FIELD_OFFSET + position * FIELD_BYTES;
        bytes[at] = 16;
        bytes[at + 11] = 18;
        bytes[at + 12] = 0;
        bytes[at + 15] = 2;
        put16(&mut bytes, at + 7, 0);
        put16(&mut bytes, at + 23, width);
        put16(&mut bytes, page * PAGE_BYTES + 43, 1);
    }
    put16(
        &mut bytes,
        FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + 2 * FIELD_BYTES + 21,
        4,
    );
    assert_eq!(
        read(&bytes).unwrap().value_column().data_type(),
        ScalarType::Decimal
    );
    for (at, value) in [
        (TEMPLATE_PAGE * PAGE_BYTES + 16, 1),
        (TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET + 11, 17),
        (TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET + 12, 1),
        (FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES + 12, 1),
        (FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES + 23, 9),
    ] {
        let mut changed = bytes.clone();
        changed[at] = value;
        assert!(read(&changed).is_err());
    }
    // Ordinary 17-byte conceptual fields still require matching field parameters.
    put16(
        &mut bytes,
        TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET + 23,
        17,
    );
    bytes[FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES + 12] = 1;
    assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
}

#[test]
fn non_multivalue_and_unknown_extended_templates_decline_even_when_empty() {
    for (kind, width) in [(5, 8), (8, 8), (9, 4)] {
        let mut bytes = links_fixture::populated();
        rows(&mut bytes, &[]);
        for at in [
            TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET,
            FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES,
        ] {
            bytes[at] = kind;
            put16(&mut bytes, at + 23, width);
        }
        put16(
            &mut bytes,
            FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + 2 * FIELD_BYTES + 21,
            4 + width,
        );
        assert!(
            read_complex_column_links(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| false
            )
            .is_ok()
        );
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
    for (offset, value) in [
        (TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET + 15, 7),
        (TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET + 16, 0x08),
    ] {
        let mut bytes = links_fixture::populated();
        rows(&mut bytes, &[]);
        bytes[offset] = value;
        assert_eq!(read(&bytes).err(), Some(InspectError::Unsupported));
    }
}

#[test]
fn collection_reads_keep_reference_failures_budget_cancellation_and_source_bytes() {
    let original = links_fixture::populated();
    let count = Cell::new(0);
    let mut source = Cursor::new(original.clone());
    read_multivalue_column(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Score",
        &mut budget(),
        &|| {
            count.set(count.get() + 1);
            false
        },
    )
    .unwrap();
    for stop in [0, count.get() / 2, count.get() - 1] {
        let calls = Cell::new(0);
        assert!(matches!(
            read_multivalue_column(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut budget(),
                &|| {
                    let value = calls.get();
                    calls.set(value + 1);
                    value >= stop
                }
            ),
            Err(InspectError::Cancelled)
        ));
    }
    for mut limited in [
        ResourceBudget::new(0, 16_384, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 16_384, 0),
    ] {
        assert!(matches!(
            read_multivalue_column(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Score",
                &mut limited,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    assert_eq!(source.into_inner(), original);
    let mut orphan = original;
    let at = CHILD_PAGE * PAGE_BYTES
        + usize::from(u16::from_le_bytes(
            orphan[CHILD_PAGE * PAGE_BYTES + 14..CHILD_PAGE * PAGE_BYTES + 16]
                .try_into()
                .unwrap(),
        ));
    put32(&mut orphan, at + 2, 999);
    assert!(matches!(
        read(&orphan),
        Err(InspectError::ComplexReference(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig {cases:128,
        rng_seed:proptest::test_runner::RngSeed::Fixed(0x4d554c544956414c),..Default::default()})]
    #[test]
    fn nullable_multivalues_keep_exact_sequence_multiplicity_and_count(values in prop::collection::vec(prop::option::of(any::<i32>()),0..=100)) {
        let mut bytes=links_fixture::populated(); rows(&mut bytes,&values);
        let collection=read(&bytes).unwrap();
        let actual=collection.values(0).unwrap().cloned().collect::<Vec<_>>();
        let expected=values.into_iter().map(|value| value.map_or(Value::Null,Value::Int32)).collect::<Vec<_>>();
        prop_assert_eq!(actual,expected);
        prop_assert_eq!(collection.values(1).unwrap().len(),0);
    }
}

#[test]
fn ace2007_collections_do_not_admit_large_number_templates() {
    let mut bytes = links_fixture::populated();
    bytes[20] = 2;
    for at in [
        TEMPLATE_PAGE * PAGE_BYTES + FIELD_OFFSET,
        FLAT_PAGE * PAGE_BYTES + FIELD_OFFSET + FIELD_BYTES,
    ] {
        bytes[at] = 0x13;
        put16(&mut bytes, at + 23, 8);
    }
    assert!(matches!(read(&bytes), Err(InspectError::Unsupported)));
}
