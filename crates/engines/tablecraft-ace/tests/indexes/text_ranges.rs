use super::{budget, copy};
use std::{
    io::Cursor,
    ops::Bound::{Excluded, Included, Unbounded},
};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, InspectError, TextIndexSelection,
    read_table_index_projection, read_table_text_index_range,
};
use tablecraft_core::values::Value;
use tablecraft_model::CatalogScope;

#[test]
fn physical_text_intervals_and_projections_keep_reference_cursor_order() {
    let original = copy();
    let mut source = Cursor::new(original.clone());
    for (selection, expected) in [
        (
            TextIndexSelection::Range {
                start: Included("Office Objects"),
                end: Included("Project Objects"),
            },
            vec![9, 8, 10, 11, 12],
        ),
        (
            TextIndexSelection::Range {
                start: Excluded("Office Objects"),
                end: Excluded("Project Objects"),
            },
            vec![8, 10, 11],
        ),
        (
            TextIndexSelection::Range {
                start: Included("Project Objects"),
                end: Included("Office Objects"),
            },
            vec![],
        ),
        (
            TextIndexSelection::Range {
                start: Included("Office Objects"),
                end: Excluded("Office Objects"),
            },
            vec![],
        ),
        (TextIndexSelection::Null, vec![]),
        (
            TextIndexSelection::Range {
                start: Unbounded,
                end: Unbounded,
            },
            vec![
                1, 2, 3, 4, 5, 6, 7, 9, 8, 10, 11, 12, 13, 14, 15, 18, 17, 16, 19,
            ],
        ),
    ] {
        let rows = read_table_text_index_range(
            &mut source,
            CatalogScope::new(1),
            "TAG_GRP_TB",
            "TAG_GRP_NME",
            selection,
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            rows.rows()
                .iter()
                .map(|row| row[0].clone())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .copied()
                .map(Value::Int32)
                .collect::<Vec<_>>()
        );
        let projected = read_table_index_projection(
            &mut source,
            CatalogScope::new(1),
            "TAG_GRP_TB",
            "TAG_GRP_NME",
            IndexProjection {
                columns: &["TAG_GRP_ID"],
                selection: IndexProjectionSelection::Text(selection),
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            projected.rows(),
            expected
                .iter()
                .copied()
                .map(|id| vec![Value::Int32(id)])
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(source.into_inner(), original);
}

#[test]
fn typed_all_null_and_empty_ranges_reject_wrong_index_profiles() {
    for selection in [
        TextIndexSelection::All,
        TextIndexSelection::Null,
        TextIndexSelection::Range {
            start: Included("z"),
            end: Included("a"),
        },
    ] {
        assert!(matches!(
            read_table_text_index_range(
                &mut Cursor::new(copy()),
                CatalogScope::new(1),
                "ROULETTE_TB",
                "ORDR_NBR",
                selection,
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
        assert!(matches!(
            read_table_index_projection(
                &mut Cursor::new(copy()),
                CatalogScope::new(1),
                "ROULETTE_TB",
                "ORDR_NBR",
                IndexProjection {
                    columns: &["ORDR_NBR"],
                    selection: IndexProjectionSelection::Text(selection)
                },
                &mut budget(),
                &|| false
            ),
            Err(InspectError::Unsupported)
        ));
    }
}

#[test]
fn oversize_utf16_endpoints_and_cancellation_fail_before_source_reads() {
    for oversized in ["a".repeat(256), "😀".repeat(128)] {
        let mut source = Cursor::new(copy());
        assert!(matches!(
            read_table_text_index_range(
                &mut source,
                CatalogScope::new(1),
                "TAG_GRP_TB",
                "TAG_GRP_NME",
                TextIndexSelection::Range {
                    start: Included(&oversized),
                    end: Unbounded
                },
                &mut budget(),
                &|| false
            ),
            Err(InspectError::StructureLimit)
        ));
        assert_eq!(source.position(), 0);
    }
    let mut source = Cursor::new(copy());
    assert!(matches!(
        read_table_text_index_range(
            &mut source,
            CatalogScope::new(1),
            "TAG_GRP_TB",
            "TAG_GRP_NME",
            TextIndexSelection::All,
            &mut budget(),
            &|| true
        ),
        Err(InspectError::Cancelled)
    ));
    assert_eq!(source.position(), 0);
}
