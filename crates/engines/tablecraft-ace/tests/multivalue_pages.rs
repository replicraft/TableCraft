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
use tablecraft_ace::{
    InspectError, MultivaluePageValue, MultivalueTablePage, TablePageRequest,
    read_multivalue_table_page, read_table_page,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

const PAGE_BYTES: usize = 4096;
const CHILD_TABLE: usize = 20;
const CHILD_DATA: usize = 22;
const PARENT_TABLE: usize = 10;
const FIRST_FIELD: usize = 63;

fn database() -> Vec<u8> {
    let mut bytes = links_fixture::populated();
    let rows = [(1i32, 10i32, 3u8), (2, 20, 3), (3, 0, 1)]
        .into_iter()
        .map(|(id, key, mask)| {
            let mut row = 2u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(key.to_le_bytes());
            row.push(mask);
            (0, row)
        })
        .collect::<Vec<_>>();
    const PARENT_DATA: usize = 12;
    bytes[PARENT_DATA * PAGE_BYTES..(PARENT_DATA + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(PARENT_TABLE as u32, &rows));
    bytes
}
fn budget() -> ResourceBudget {
    ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000)
}
fn read(bytes: &[u8], offset: u32, limit: u32) -> Result<MultivalueTablePage, InspectError> {
    read_multivalue_table_page(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        TablePageRequest::new(offset, limit).unwrap(),
        false,
        &mut budget(),
        &|| false,
    )
}
fn child_rows(bytes: &mut [u8], values: &[Option<i32>]) {
    support::put32(bytes, CHILD_TABLE * PAGE_BYTES + 16, values.len() as u32);
    let rows = values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            let mut row = 3u16.to_le_bytes().to_vec();
            row.extend(10i32.to_le_bytes());
            row.extend(value.unwrap_or(0).to_le_bytes());
            row.extend((i as i32 + 1).to_le_bytes());
            row.push(if value.is_some() { 7 } else { 5 });
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[CHILD_DATA * PAGE_BYTES..(CHILD_DATA + 1) * PAGE_BYTES]
        .copy_from_slice(&support::data_page(CHILD_TABLE as u32, &rows));
}
fn collection(page: &MultivalueTablePage, row: usize) -> (Option<i32>, Vec<Value>) {
    match page.cell(row, 1).unwrap() {
        MultivaluePageValue::Collection(value) => {
            (value.key(), value.values().unwrap().cloned().collect())
        }
        _ => panic!("Expected explicit collection"),
    }
}

#[test]
fn mixed_pages_keep_scalar_cells_duplicates_null_elements_and_empty_key_states() {
    for code in [2, 5, 6] {
        let mut bytes = database();
        bytes[20] = code;
        child_rows(&mut bytes, &[Some(-40), None, Some(-40)]);
        let page = read(&bytes, 0, 4).unwrap();
        assert_eq!(page.row_count(), 3);
        assert!(page.reached_end());
        assert!(page.features().is_none());
        assert_eq!(page.schema().columns()[1].native_type(), 0x12);
        assert!(matches!(
            page.cell(0, 0),
            Some(MultivaluePageValue::Scalar(Value::Int32(1)))
        ));
        assert_eq!(
            collection(&page, 0),
            (
                Some(10),
                vec![Value::Int32(-40), Value::Null, Value::Int32(-40)]
            )
        );
        assert_eq!(collection(&page, 1), (Some(20), vec![]));
        assert_eq!(collection(&page, 2), (None, vec![]));
        assert!(page.collection_column(0).is_none());
        assert!(page.collection_column(1).is_some());
        assert!(page.cell(3, 1).is_none());
        assert!(page.cell(0, 2).is_none());
        assert!(matches!(
            read_table_page(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                TablePageRequest::new(0, 1).unwrap(),
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
        let selected = read(&bytes, 1, 1).unwrap();
        assert_eq!(collection(&selected, 0), (Some(20), vec![]));
        assert!(!selected.reached_end());
        assert_eq!(selected.request(), TablePageRequest::new(1, 1).unwrap());
        assert!(read(&bytes, u32::MAX, 1).unwrap().reached_end());
    }
}

#[test]
fn orphan_children_and_unknown_scalar_columns_fail_even_for_an_empty_requested_page() {
    let mut bytes = database();
    child_rows(&mut bytes, &[Some(7)]);
    let row = u16::from_le_bytes(
        bytes[CHILD_DATA * PAGE_BYTES + 14..CHILD_DATA * PAGE_BYTES + 16]
            .try_into()
            .unwrap(),
    ) as usize;
    bytes[CHILD_DATA * PAGE_BYTES + row + 2..CHILD_DATA * PAGE_BYTES + row + 6]
        .copy_from_slice(&999i32.to_le_bytes());
    assert!(read(&bytes, u32::MAX, 1).is_err());
    let mut bytes = database();
    bytes[PARENT_TABLE * PAGE_BYTES + FIRST_FIELD] = u8::MAX;
    assert!(matches!(read(&bytes, 0, 1), Err(InspectError::Unsupported)));
}

#[test]
fn whole_collection_limits_are_enforced_instead_of_silently_truncating_values() {
    let mut bytes = database();
    child_rows(&mut bytes, &(0..100).map(Some).collect::<Vec<_>>());
    assert_eq!(collection(&read(&bytes, 0, 1).unwrap(), 0).1.len(), 100);
    child_rows(&mut bytes, &(0..101).map(Some).collect::<Vec<_>>());
    assert!(matches!(
        read(&bytes, u32::MAX, 1),
        Err(InspectError::StructureLimit)
    ));
}

#[test]
fn explicit_multivalue_pages_retain_budget_and_final_cancellation_checks() {
    let bytes = database();
    assert!(matches!(
        read_multivalue_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            false,
            &mut ResourceBudget::new(0, 0, 0),
            &|| false
        ),
        Err(InspectError::Limit(_))
    ));
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    read_multivalue_table_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        TablePageRequest::new(0, 1).unwrap(),
        false,
        &mut budget(),
        &count,
    )
    .unwrap();
    let total = calls.get();
    calls.set(0);
    let cancel = || {
        calls.set(calls.get() + 1);
        calls.get() == total
    };
    assert!(matches!(
        read_multivalue_table_page(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            TablePageRequest::new(0, 1).unwrap(),
            false,
            &mut budget(),
            &cancel
        ),
        Err(InspectError::Cancelled)
    ));
}

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 128,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4d56504147455331),
        ..Default::default()
    })]
    #[test]
    fn generated_collections_keep_order_and_membership_under_parent_paging(
        values in proptest::collection::vec(proptest::option::of(any::<i32>()), 0..16),
        offset in 0u32..5,
        limit in 1u32..4,
    ) {
        let mut bytes = database();
        child_rows(&mut bytes, &values);
        let page = read(&bytes, offset, limit).unwrap();
        let expected_count = 3usize.saturating_sub(offset as usize).min(limit as usize);
        prop_assert_eq!(page.row_count(), expected_count);
        if offset == 0 {
            let expected = values.into_iter()
                .map(|value| value.map(Value::Int32).unwrap_or(Value::Null))
                .collect::<Vec<_>>();
            prop_assert_eq!(collection(&page, 0).1, expected);
        }
        for row in 0..page.row_count() {
            match offset as usize + row {
                1 => prop_assert_eq!(collection(&page, row), (Some(20), vec![])),
                2 => prop_assert_eq!(collection(&page, row), (None, vec![])),
                _ => {},
            }
        }
    }
}

#[test]
fn general_collection_pages_preserve_multivalue_null_keys_and_partial_ranges() {
    use tablecraft_ace::{
        StoredCollectionItem, StoredCollectionPageValue, read_stored_collection_table_page,
    };
    let mut bytes = database();
    child_rows(&mut bytes, &[Some(12), None, Some(12)]);
    let page = read_stored_collection_table_page(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        TablePageRequest::new(0, 3).unwrap(),
        false,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let StoredCollectionPageValue::Collection(first) = page.cell(0, 1).unwrap() else {
        panic!("Expected multivalue")
    };
    let values = first
        .items()
        .unwrap()
        .map(|item| match item.unwrap() {
            StoredCollectionItem::Value(value) => value.clone(),
            _ => panic!("Wrong collection type"),
        })
        .collect::<Vec<_>>();
    assert_eq!(values, [Value::Int32(12), Value::Null, Value::Int32(12)]);
    let StoredCollectionPageValue::Collection(absent) = page.cell(2, 1).unwrap() else {
        panic!("Expected absent key")
    };
    assert_eq!(absent.key(), None);
    assert_eq!(absent.items().unwrap().len(), 0);
    assert!(page.cell(0, 2).is_none());
}
