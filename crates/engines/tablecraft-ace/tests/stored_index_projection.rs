#[allow(dead_code)]
#[path = "calculated_storage/fixture.rs"]
mod calculated_fixture;
#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod fixture;
#[path = "stored_index_projection/fixture.rs"]
mod indexed;
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom},
    ops::Bound,
};
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, IndexedTableSnapshot, InspectError,
    Int32IndexSelection, NativeReadSource, read_stored_table_index_with_references,
    read_stored_table_index_with_request, read_table_index_with_references,
    read_table_index_with_request,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const BYTES: u64 = 64 * 1024 * 1024;

fn budget() -> ResourceBudget {
    ResourceBudget::new(BYTES, 100_000, 1_000_000)
}

fn request<'a>(columns: Option<&'a [&'a str]>) -> IndexReadRequest<'a> {
    IndexReadRequest {
        columns,
        selection: IndexProjectionSelection::All,
        maximum_rows: 3,
    }
}

fn read(bytes: &[u8], names: &[&str]) -> Result<IndexedTableSnapshot, InspectError> {
    read_stored_table_index_with_references(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "IdOrder",
        request(Some(names)),
        &mut budget(),
        &|| false,
    )
}

#[test]
fn cached_outputs_keep_stale_null_values_actual_order_aliases_and_ordinary_key_checks() {
    for profile in [5, 6] {
        for descending in [false, true] {
            let mut bytes = indexed::database(&[Some(77), None, Some(-4)], descending, true);
            bytes[20] = profile;
            let original = bytes.clone();
            let mut expected = vec![
                vec![Value::Int32(77)],
                vec![Value::Null],
                vec![Value::Int32(-4)],
            ];
            let mut slots = [0, 1, 2];
            if descending {
                expected.reverse();
                slots.reverse();
            }
            for name in ["IdOrder", "IdTwin"] {
                let result = read_stored_table_index_with_references(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    name,
                    request(Some(&["Score"])),
                    &mut budget(),
                    &|| false,
                )
                .unwrap();
                assert_eq!(result.snapshot().rows(), expected);
                assert_eq!(
                    result
                        .row_references()
                        .iter()
                        .map(|value| (value.page_number(), value.slot_number()))
                        .collect::<Vec<_>>(),
                    slots.iter().map(|&slot| (12, slot)).collect::<Vec<_>>()
                );
            }
            let all = read_stored_table_index_with_request(
                &mut Cursor::new(&bytes),
                CatalogScope::new(1),
                "Items",
                "IdOrder",
                request(None),
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(all.columns().len(), 2);
            assert!(all.schema().columns()[1].scalar_type().is_none());
            assert!(matches!(
                read_table_index_with_request(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "IdOrder",
                    request(Some(&["Score"])),
                    &mut budget(),
                    &|| false,
                ),
                Err(InspectError::Unsupported)
            ));
            assert!(matches!(
                read_table_index_with_references(
                    &mut Cursor::new(&bytes),
                    CatalogScope::new(1),
                    "Items",
                    "IdOrder",
                    request(Some(&["Score"])),
                    &mut budget(),
                    &|| false,
                ),
                Err(InspectError::Unsupported)
            ));
            assert_eq!(bytes, original);
        }
    }
}

#[test]
fn malformed_selected_cache_fails_but_unselected_cache_is_not_decoded() {
    let original = indexed::database(&[Some(77), Some(88), Some(99)], false, false);
    let mut bytes = original.clone();
    let start = indexed::row_start(&bytes, 1);
    bytes[start + 6] = 1;
    assert!(matches!(
        read(&bytes, &["Score"]),
        Err(InspectError::Unsupported)
    ));
    assert_eq!(
        read(&bytes, &["Id"]).unwrap().snapshot().rows(),
        &[
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)],
        ]
    );
    let mut wrong_key = original;
    let start = indexed::row_start(&wrong_key, 1);
    wrong_key[start + 2..start + 6].copy_from_slice(&40_i32.to_le_bytes());
    assert!(matches!(
        read(&wrong_key, &["Score"]),
        Err(InspectError::Malformed)
    ));
}

#[test]
fn empty_declarations_calculated_keys_and_nonall_selections_remain_closed() {
    let empty = indexed::database(&[], false, false);
    assert!(
        read(&empty, &["Score"])
            .unwrap()
            .snapshot()
            .rows()
            .is_empty()
    );
    let mut wrong_result = empty.clone();
    let property = wrong_result
        .windows(9)
        .position(|value| value == [9, 0, 0, 2, 1, 0, 1, 0, 4])
        .unwrap();
    wrong_result[property + 8] = 8;
    assert!(matches!(
        read(&wrong_result, &["Score"]),
        Err(InspectError::Unsupported)
    ));
    let mut missing_expression = empty.clone();
    let name: Vec<u8> = "Expression"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let at = missing_expression
        .windows(name.len())
        .position(|value| value == name)
        .unwrap();
    missing_expression[at] = b'X';
    assert!(matches!(
        read(&missing_expression, &["Score"]),
        Err(InspectError::Unsupported)
    ));
    let mut wrong_layout = empty;
    wrong_layout[10 * indexed::PAGE + 75 + 25 + 23] = 38;
    assert!(matches!(
        read(&wrong_layout, &["Score"]),
        Err(InspectError::Unsupported)
    ));
    let mut key = indexed::database(&[Some(77), None, Some(-4)], false, false);
    support::put16(&mut key, 10 * indexed::PAGE + 147, 1);
    assert!(matches!(
        read(&key, &["Score"]),
        Err(InspectError::Unsupported)
    ));
    let bytes = indexed::database(&[Some(77), None, Some(-4)], false, false);
    assert!(matches!(
        read_stored_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            IndexReadRequest {
                selection: IndexProjectionSelection::Int32(Int32IndexSelection::Range {
                    lower: Bound::Included(1),
                    upper: Bound::Included(2)
                }),
                ..request(Some(&["Score"]))
            },
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Unsupported)
    ));
}

struct SchemaDrift {
    bytes: Cursor<Vec<u8>>,
    definition_reads: usize,
    data_reads: usize,
}
impl NativeReadSource for SchemaDrift {}
impl Read for SchemaDrift {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let position = self.bytes.position();
        let count = self.bytes.read(output)?;
        if position == (10 * indexed::PAGE) as u64 {
            self.definition_reads += 1;
            if self.definition_reads == 2 && count > 96 {
                output[75 + 21] ^= 2;
            }
        }
        if position == (12 * indexed::PAGE) as u64 {
            self.data_reads += 1;
        }
        Ok(count)
    }
}
impl Seek for SchemaDrift {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn forwarded_cached_rows_keep_logical_references_and_schema_drift_fails_before_data() {
    let bytes = indexed::forwarded(indexed::database(&[Some(77), None, Some(-4)], false, false));
    let result = read(&bytes, &["Score"]).unwrap();
    assert_eq!(
        result
            .row_references()
            .iter()
            .map(|reference| (reference.page_number(), reference.slot_number()))
            .collect::<Vec<_>>(),
        [(12, 0), (12, 1), (12, 2)]
    );
    assert_eq!(result.snapshot().rows()[0], [Value::Int32(77)]);
    let mut changing = SchemaDrift {
        bytes: Cursor::new(bytes),
        definition_reads: 0,
        data_reads: 0,
    };
    assert!(matches!(
        read_stored_table_index_with_request(
            &mut changing,
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(Some(&["Score"])),
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::Malformed)
    ));
    // Features capture one schema; index inspection then rereads that schema
    // and its physical index declaration before the binding comparison runs.
    assert_eq!(changing.definition_reads, 3);
    assert_eq!(changing.data_reads, 0);
}

#[test]
fn stored_reference_allocation_caps_and_late_cancellation_remain_paid() {
    let bytes = indexed::database(&[Some(77), None, Some(-4)], false, false);
    let mut measured = budget();
    read_stored_table_index_with_request(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "IdOrder",
        request(Some(&["Score"])),
        &mut measured,
        &|| false,
    )
    .unwrap();
    let consumed = BYTES - measured.remaining(Resource::Bytes);
    assert!(matches!(
        read_stored_table_index_with_references(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(Some(&["Score"])),
            &mut ResourceBudget::new(consumed, 100_000, 1_000_000),
            &|| false,
        ),
        Err(InspectError::Limit(_))
    ));
    assert!(matches!(
        read_stored_table_index_with_request(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            IndexReadRequest {
                maximum_rows: 2,
                ..request(Some(&["Score"]))
            },
            &mut budget(),
            &|| false,
        ),
        Err(InspectError::StructureLimit)
    ));
    let calls = Cell::new(0);
    read_stored_table_index_with_references(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "IdOrder",
        request(Some(&["Score"])),
        &mut budget(),
        &|| {
            calls.set(calls.get() + 1);
            false
        },
    )
    .unwrap();
    let terminal = calls.get() - 1;
    calls.set(0);
    assert!(matches!(
        read_stored_table_index_with_references(
            &mut Cursor::new(&bytes),
            CatalogScope::new(1),
            "Items",
            "IdOrder",
            request(Some(&["Score"])),
            &mut budget(),
            &|| {
                let count = calls.get();
                calls.set(count + 1);
                count == terminal
            },
        ),
        Err(InspectError::Cancelled)
    ));
}
