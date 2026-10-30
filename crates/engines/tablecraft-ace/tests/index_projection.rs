use tablecraft_testkit::access::mutation_fixture as fixture;
#[path = "mutation/index_fixture.rs"]
mod index_fixture;
#[path = "index_projection/profiles.rs"]
mod profiles;
use std::{io::Cursor, ops::Bound};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, InspectError, Int32IndexSelection,
    Int64IndexSelection, TableSnapshot, read_table_index_projection,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;
use tablecraft_testkit::access as support;

const PAGE_BYTES: usize = 4096;
const DEFINITION_PAGE: usize = 10;
const FIELD_START: usize = 75;
const DATA_PAGE: usize = 12;
const SECOND_SLOT: usize = 16;
const SCORE_OFFSET: usize = 6;
fn read(
    bytes: &[u8],
    fields: &[&str],
    selection: IndexProjectionSelection,
) -> Result<TableSnapshot, InspectError> {
    read_table_index_projection(
        &mut Cursor::new(bytes),
        CatalogScope::new(1),
        "Items",
        "ScoreKey",
        IndexProjection {
            columns: fields,
            selection,
        },
        &mut ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
        &|| false,
    )
}
#[test]
fn indexed_projections_validate_hidden_keys_and_preserve_native_order() {
    for descending in [false, true] {
        for nullable in [false, true] {
            let original = index_fixture::database(descending, false, nullable);
            let ids = read(&original, &["Id"], IndexProjectionSelection::All).unwrap();
            assert_eq!(ids.columns().len(), 1);
            assert_eq!(ids.columns()[0].name(), "Id");
            assert_eq!(ids.rows().len(), 3);
            let empty = read(&original, &[], IndexProjectionSelection::All).unwrap();
            assert!(empty.columns().is_empty());
            assert_eq!(empty.rows(), &[vec![], vec![], vec![]]);
            let mut mixed = original.clone();
            mixed[DEFINITION_PAGE * PAGE_BYTES + FIELD_START] = 0xff;
            let scalar = read(&mixed, &["Score"], IndexProjectionSelection::All).unwrap();
            let mut expected = vec![
                vec![if nullable {
                    Value::Null
                } else {
                    Value::Int32(10)
                }],
                vec![Value::Int32(20)],
                vec![Value::Int32(30)],
            ];
            if descending {
                expected.reverse();
            }
            assert_eq!(scalar.rows(), expected);
            let range = read(
                &mixed,
                &["Score"],
                IndexProjectionSelection::Int32(Int32IndexSelection::Range {
                    lower: Bound::Included(20),
                    upper: Bound::Excluded(30),
                }),
            )
            .unwrap();
            assert_eq!(range.rows(), &[vec![Value::Int32(20)]]);
            assert!(matches!(
                read(&mixed, &["Id"], IndexProjectionSelection::All),
                Err(InspectError::Unsupported)
            ));
            assert!(matches!(
                read(
                    &mixed,
                    &["Score"],
                    IndexProjectionSelection::Int64(Int64IndexSelection::All)
                ),
                Err(InspectError::Unsupported)
            ));
            let mut corrupt = original.clone();
            let row = usize::from(u16::from_le_bytes(
                corrupt[DATA_PAGE * PAGE_BYTES + SECOND_SLOT
                    ..DATA_PAGE * PAGE_BYTES + SECOND_SLOT + 2]
                    .try_into()
                    .unwrap(),
            ));
            corrupt[DATA_PAGE * PAGE_BYTES + row + SCORE_OFFSET
                ..DATA_PAGE * PAGE_BYTES + row + SCORE_OFFSET + 4]
                .copy_from_slice(&21_i32.to_le_bytes());
            assert!(matches!(
                read(&corrupt, &["Id"], IndexProjectionSelection::All),
                Err(InspectError::Malformed)
            ));
            assert!(matches!(
                read(&corrupt, &[], IndexProjectionSelection::All),
                Err(InspectError::Malformed)
            ));
            assert!(read(&original, &["Id", "Id"], IndexProjectionSelection::All).is_err());
        }
    }
}

#[test]
fn projected_node_ranges_validate_only_the_selected_subtree_and_rows() {
    let mut bytes = index_fixture::node_database();
    assert_eq!(
        read(&bytes, &["Id"], IndexProjectionSelection::All)
            .unwrap()
            .rows(),
        &[
            vec![Value::Int32(1)],
            vec![Value::Int32(2)],
            vec![Value::Int32(3)]
        ]
    );
    const DISJOINT_LEAF: usize = 15;
    bytes[DISJOINT_LEAF * PAGE_BYTES] = 0;
    let selected = read(
        &bytes,
        &["Id"],
        IndexProjectionSelection::Int32(Int32IndexSelection::Range {
            lower: Bound::Included(30),
            upper: Bound::Included(30),
        }),
    )
    .unwrap();
    assert_eq!(selected.rows(), &[vec![Value::Int32(3)]]);
    assert!(read(&bytes, &["Id"], IndexProjectionSelection::All).is_err());
}
