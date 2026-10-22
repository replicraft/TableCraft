use super::{budget, database, support};
use std::{io::Cursor, ops::Bound::Included};
use tablecraft_ace::{
    IndexProjectionSelection, IndexReadRequest, InspectError, Int32IndexSelection,
    read_table_index, read_table_index_with_request,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

fn read(
    bytes: &[u8],
    maximum_rows: u32,
    columns: Option<&[&str]>,
    selection: IndexProjectionSelection<'_>,
) -> Result<tablecraft_ace::TableSnapshot, InspectError> {
    read_table_index_with_request(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        IndexReadRequest {
            columns,
            selection,
            maximum_rows,
        },
        &mut budget(),
        &|| false,
    )
}

#[test]
fn invalid_limits_fail_before_reading_an_empty_source() {
    for maximum_rows in [0, 1_000_001] {
        assert_eq!(
            read(&[], maximum_rows, None, IndexProjectionSelection::All).err(),
            Some(InspectError::InvalidSnapshotLimits)
        );
    }
}

#[test]
fn complete_and_zero_width_reads_keep_exact_retention_caps() {
    let bytes = database();
    let expected = read_table_index(
        &mut Cursor::new(&bytes),
        CatalogScope::new(1),
        "Items",
        "Compound",
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        read(&bytes, 2, None, IndexProjectionSelection::All)
            .unwrap()
            .rows(),
        expected.rows()
    );
    assert_eq!(
        read(&bytes, 2, Some(&[]), IndexProjectionSelection::All)
            .unwrap()
            .rows(),
        &[Vec::<Value>::new(), Vec::<Value>::new()]
    );
    assert_eq!(
        read(&bytes, 1, None, IndexProjectionSelection::All).err(),
        Some(InspectError::StructureLimit)
    );
}

#[test]
fn partial_reads_cap_matches_without_certifying_a_larger_declaration() {
    let mut bytes = database();
    // Deliberately inconsistent metadata: a seek must not certify the whole table.
    support::put32(&mut bytes, 10 * 4096 + 16, 20_000);
    let selection = IndexProjectionSelection::Int32(Int32IndexSelection::Range {
        lower: Included(2),
        upper: Included(2),
    });
    assert_eq!(
        read(&bytes, 1, None, selection).unwrap().rows(),
        &[vec![Value::Int32(2), Value::Int32(8)]]
    );
    assert_eq!(
        read(&bytes, 1, None, IndexProjectionSelection::All).err(),
        Some(InspectError::StructureLimit)
    );
    assert!(read(&bytes, 20_000, None, IndexProjectionSelection::All).is_err());
    let both = IndexProjectionSelection::Int32(Int32IndexSelection::Range {
        lower: Included(1),
        upper: Included(2),
    });
    assert_eq!(
        read(&bytes, 1, None, both).err(),
        Some(InspectError::StructureLimit)
    );
}
