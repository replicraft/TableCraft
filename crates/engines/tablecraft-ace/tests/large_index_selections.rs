#[allow(dead_code)]
#[path = "mutation/fixture.rs"]
mod base;
#[allow(dead_code)]
#[path = "composite_indexes/fixture.rs"]
mod composite_fixture;
// Shared with the independent complete-request contract target.
#[allow(dead_code)]
#[path = "large_index_selections/fixture.rs"]
mod fixture;
mod support;

use std::ops::Bound::{Included, Unbounded};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, IndexReadRequest, InspectError, Int32IndexSelection,
    read_table_index, read_table_index_in_table_order, read_table_index_projection,
    read_table_index_range, read_table_index_with_request,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::CatalogScope;

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024, 1024, 1_000_000)
}

fn point(source: &mut fixture::Sparse, id: i32) -> Result<Vec<Vec<Value>>, InspectError> {
    read_table_index_range(
        source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        Int32IndexSelection::Range {
            lower: Included(id),
            upper: Included(id),
        },
        &mut budget(),
        &|| false,
    )
    .map(|snapshot| snapshot.rows().to_vec())
}

#[test]
fn selected_rows_use_large_inline_and_indirect_ownership_without_reading_unused_pages() {
    for code in [5, 6] {
        for indirect in [false, true] {
            for id in [1, 2, 3] {
                let mut source = fixture::Sparse::new(code, indirect);
                let rows = point(&mut source, id).unwrap();
                assert_eq!(rows, [vec![Value::Int32(id), Value::Int32(id * 10)]]);
                assert_eq!(source.data_reads, 1);
                assert!(source.bytes_read < 256 * 1024);
            }
        }
    }
}

#[test]
fn projected_table_order_stays_independent_of_index_order_and_hidden_key_positions() {
    for code in [5, 6] {
        let mut source = fixture::Sparse::new(code, true);
        let result = read_table_index_in_table_order(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            IndexReadRequest {
                columns: Some(&["Score"]),
                selection: IndexProjectionSelection::Int32(Int32IndexSelection::Range {
                    lower: Unbounded,
                    upper: Unbounded,
                }),
                maximum_rows: 3,
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Int32(30)],
                vec![Value::Int32(10)],
                vec![Value::Int32(20)]
            ]
        );
        assert_eq!(source.data_reads, 1);
    }
}

#[test]
fn no_match_still_preflights_late_bitmap_metadata_before_any_selected_record() {
    let mut source = fixture::Sparse::new(6, true);
    source.bad_second_bitmap();
    assert_eq!(point(&mut source, 99), Err(InspectError::Malformed));
    assert_eq!(source.data_reads, 0);

    let mut source = fixture::Sparse::new(6, false);
    assert_eq!(point(&mut source, 99).unwrap(), Vec::<Vec<Value>>::new());
    assert_eq!(source.data_reads, 0);
}

#[test]
fn missing_ownership_wrong_owner_and_row_key_mismatch_never_fall_back() {
    #[derive(Clone, Copy, Debug)]
    enum Fault {
        MissingBit,
        WrongOwner,
        WrongKey,
    }
    for fault in [Fault::MissingBit, Fault::WrongOwner, Fault::WrongKey] {
        let mut source = fixture::Sparse::new(6, true);
        match fault {
            Fault::MissingBit => source.remove_selected_membership(),
            Fault::WrongOwner => source.wrong_owner(),
            Fault::WrongKey => source.wrong_key(),
        }
        assert_eq!(
            point(&mut source, 1),
            Err(InspectError::Malformed),
            "{fault:?}"
        );
        assert_eq!(
            source.data_reads,
            usize::from(!matches!(fault, Fault::MissingBit))
        );
    }
}

#[test]
fn complete_materialized_reads_keep_the_existing_map_ceiling() {
    let mut source = fixture::Sparse::new(6, true);
    assert_eq!(
        read_table_index(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            &mut budget(),
            &|| false
        )
        .err(),
        Some(InspectError::StructureLimit),
    );
    assert_eq!(source.data_reads, 0);
}

#[test]
fn cancellation_and_exhausted_allowances_publish_no_selected_rows() {
    let mut source = fixture::Sparse::new(6, true);
    assert_eq!(
        read_table_index_range(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            Int32IndexSelection::Range {
                lower: Included(1),
                upper: Included(1)
            },
            &mut budget(),
            &|| true
        )
        .err(),
        Some(InspectError::Cancelled),
    );
    for allowance in [
        ResourceBudget::new(1, 1024, 1_000_000),
        ResourceBudget::new(2 * 1024 * 1024, 0, 1_000_000),
        ResourceBudget::new(2 * 1024 * 1024, 1024, 0),
    ] {
        let mut source = fixture::Sparse::new(6, true);
        let mut allowance = allowance;
        assert!(matches!(
            read_table_index_range(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Compound",
                Int32IndexSelection::Range {
                    lower: Included(1),
                    upper: Included(1)
                },
                &mut allowance,
                &|| false,
            ),
            Err(InspectError::Limit(_))
        ));
    }
}

#[test]
fn explicit_complete_snapshots_preserve_counts_keys_and_native_index_order() {
    for code in [5, 6] {
        for indirect in [false, true] {
            let mut source = fixture::Sparse::new(code, indirect);
            let result = read_table_index_with_request(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Compound",
                IndexReadRequest {
                    columns: None,
                    selection: IndexProjectionSelection::All,
                    maximum_rows: 3,
                },
                &mut budget(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                result.rows(),
                [
                    vec![Value::Int32(1), Value::Int32(10)],
                    vec![Value::Int32(2), Value::Int32(20)],
                    vec![Value::Int32(3), Value::Int32(30)],
                ]
            );
            assert_eq!(source.data_reads, 1);
        }
    }
}

#[test]
fn explicit_complete_projection_uses_table_order_without_widening_legacy_projection() {
    for code in [5, 6] {
        let mut source = fixture::Sparse::new(code, true);
        let result = read_table_index_in_table_order(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            IndexReadRequest {
                columns: Some(&["Score"]),
                selection: IndexProjectionSelection::All,
                maximum_rows: 3,
            },
            &mut budget(),
            &|| false,
        )
        .unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Int32(30)],
                vec![Value::Int32(10)],
                vec![Value::Int32(20)]
            ]
        );
        assert_eq!(source.data_reads, 1);

        let mut source = fixture::Sparse::new(code, true);
        assert_eq!(
            read_table_index_projection(
                &mut source,
                CatalogScope::new(1),
                "Items",
                "Compound",
                IndexProjection {
                    columns: &["Score"],
                    selection: IndexProjectionSelection::All,
                },
                &mut budget(),
                &|| false,
            )
            .err(),
            Some(InspectError::StructureLimit)
        );
        assert_eq!(source.data_reads, 0);
    }
}
