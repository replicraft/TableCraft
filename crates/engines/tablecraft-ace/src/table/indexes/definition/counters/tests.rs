use super::*;
use crate::{
    inspect_table_indexes, inspect_table_schema,
    test_support::{index_fixture, support},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

const PAGE: usize = 4096;
const DEFINITION: usize = 10;
const FIRST_COUNTER: usize = 10 * 4096 + 67;

fn budget() -> ResourceBudget {
    ResourceBudget::new(8 * 1024 * 1024, 100_000, 1_000_000)
}

fn source(code: u8, value: u32) -> Vec<u8> {
    let mut bytes = index_fixture::database(false, true, false);
    if code != 1 {
        bytes[4..20].copy_from_slice(b"Standard ACE DB\0");
    }
    bytes[20] = code;
    support::put32(&mut bytes, FIRST_COUNTER, value);
    bytes
}

fn schema(bytes: &[u8]) -> TableSchema {
    inspect_table_schema(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap()
}

fn captured(
    bytes: &[u8],
    schema: &TableSchema,
    physical: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<IndexHistoricalCounter, InspectError> {
    let mut source = Cursor::new(bytes);
    let mut reader = Reader {
        source: &mut source,
        header: schema.header,
        budget,
        cancelled,
        reads: 0,
    };
    read_counter(&mut reader, schema, physical)
}

#[test]
fn physical_counter_keeps_literal_span_and_historical_value_across_existing_formats() {
    for code in [1, 2, 3, 5, 6] {
        for value in [0, 17, u32::MAX] {
            let bytes = source(code, value);
            let schema = schema(&bytes);
            assert_eq!(schema.declared_rows(), 3);
            let counter = captured(&bytes, &schema, 0, &mut budget(), &|| false).unwrap();
            assert_eq!(counter.definition, 10);
            assert_eq!(counter.physical, 0);
            assert_eq!(counter.range, 41027..41031);
            assert_eq!(counter.value, value);
            assert_eq!(bytes[counter.range], value.to_le_bytes());
        }
    }
}

#[test]
fn logical_aliases_resolve_to_one_original_physical_counter_without_live_count_repair() {
    let bytes = source(6, 0x01020304);
    let inspection = inspect_table_indexes(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(inspection.indexes().len(), 2);
    let schema = inspection.schema();
    let counters = inspection
        .indexes()
        .iter()
        .map(|index| {
            assert_eq!(index.data_number(), 0);
            captured(&bytes, schema, index.data_number(), &mut budget(), &|| {
                false
            })
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(counters[0], counters[1]);
    assert_eq!(counters[0].value, 0x01020304);
    assert_ne!(counters[0].value, schema.declared_rows());
    assert_eq!(&bytes[FIRST_COUNTER..FIRST_COUNTER + 4], &[4, 3, 2, 1]);
}

#[test]
fn missing_physical_slot_stale_count_short_definition_and_bad_chain_return_no_counter() {
    let original = source(6, 17);
    let schema = schema(&original);
    assert_eq!(
        captured(&original, &schema, 1, &mut budget(), &|| false),
        Err(InspectError::Malformed),
    );
    for (offset, value, expected) in [
        (DEFINITION * PAGE + 51, 2, InspectError::Malformed),
        (DEFINITION * PAGE + 8, 60, InspectError::Truncated),
        (DEFINITION * PAGE + 4, 10, InspectError::Malformed),
    ] {
        let mut bytes = original.clone();
        support::put32(&mut bytes, offset, value);
        assert_eq!(
            captured(&bytes, &schema, 0, &mut budget(), &|| false),
            Err(expected),
        );
    }
    assert_eq!(
        captured(&original, &schema, 0, &mut budget(), &|| false)
            .unwrap()
            .value,
        17
    );
}

#[test]
fn counter_capture_obeys_bytes_steps_and_cancellation_and_retries_on_same_source() {
    let bytes = source(5, 23);
    let original = bytes.clone();
    let schema = schema(&bytes);
    for resource in [Resource::Bytes, Resource::Steps] {
        let mut limited = match resource {
            Resource::Bytes => ResourceBudget::new(0, 100_000, 1_000_000),
            Resource::Steps => ResourceBudget::new(8 * 1024 * 1024, 100_000, 0),
            _ => unreachable!(),
        };
        assert!(matches!(
            captured(&bytes, &schema, 0, &mut limited, &|| false),
            Err(InspectError::Limit(LimitError { resource: actual, .. })) if actual == resource
        ));
    }
    assert_eq!(
        captured(&bytes, &schema, 0, &mut budget(), &|| true),
        Err(InspectError::Cancelled)
    );
    let calls = Cell::new(0);
    let cancelled = || {
        let current = calls.get();
        calls.set(current + 1);
        current >= 3
    };
    assert_eq!(
        captured(&bytes, &schema, 0, &mut budget(), &cancelled),
        Err(InspectError::Cancelled)
    );
    assert_eq!(bytes, original);
    let counter = captured(&bytes, &schema, 0, &mut budget(), &|| false).unwrap();
    assert_eq!(counter.value, 23);
    assert_eq!(counter.range, FIRST_COUNTER..FIRST_COUNTER + 4);
}

#[test]
fn explicit_inspection_selects_actual_logical_names_and_preserves_alias_identity() {
    let bytes = source(6, 29);
    let mut results = Vec::new();
    for name in ["ScoreKey", "scorealias"] {
        let counter = crate::inspect_table_index_counter(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            name,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(counter.definition_page(), 10);
        assert_eq!(counter.physical_index(), 0);
        assert_eq!(counter.value(), 29);
        assert_eq!(counter.byte_range(), 41027..41031);
        results.push(counter);
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(
        crate::inspect_table_index_counter(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "MissingIndex",
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Unsupported),
    );
}
