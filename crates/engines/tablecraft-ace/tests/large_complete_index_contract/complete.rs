use super::*;
use tablecraft_ace::read_table_index_in_table_order;
use tablecraft_core::values::Value;

#[test]
fn complete_population_has_exact_count_values_and_distinct_key_and_table_orders() {
    let mut source = fixture::Sparse::new(6, true);
    let snapshot = complete(&mut source).unwrap();
    let expected = [1, 2, 3].map(|id| vec![Value::Int32(id), Value::Int32(id * 10)]);
    assert_eq!(snapshot.rows(), expected);
    assert_eq!(snapshot.rows().len(), 3);
    assert_eq!(source.data_reads, 1);

    let mut source = fixture::Sparse::new(6, true);
    let request = IndexReadRequest {
        columns: Some(&["Score"]),
        ..complete_request(3)
    };
    let snapshot = read_table_index_in_table_order(
        &mut source,
        CatalogScope::new(1),
        "Items",
        "Compound",
        request,
        &mut budget(),
        &|| false,
    )
    .unwrap();
    assert_eq!(
        snapshot.rows(),
        [
            vec![Value::Int32(30)],
            vec![Value::Int32(10)],
            vec![Value::Int32(20)]
        ]
    );
    assert_eq!(source.data_reads, 1);
}

#[test]
fn exact_complete_count_mismatches_fail_before_fetching_any_indexed_root() {
    for declared in [2, 4] {
        let mut source = fixture::Sparse::new(6, true);
        source.declared_rows(declared);
        let result = read_table_index_with_request(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            complete_request(4),
            &mut budget(),
            &|| false,
        );
        assert_eq!(
            result.err(),
            Some(InspectError::Malformed),
            "declared={declared}"
        );
        assert_eq!(source.data_reads, 0);
    }
}

#[test]
fn caller_caps_are_checked_without_turning_a_complete_request_into_a_selection() {
    for (declared, cap) in [(3, 2), (5000, 4999)] {
        let mut source = fixture::Sparse::new(6, true);
        source.declared_rows(declared);
        let result = read_table_index_with_request(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            complete_request(cap),
            &mut budget(),
            &|| false,
        );
        assert_eq!(result.err(), Some(InspectError::StructureLimit));
        assert_eq!(source.data_reads, 0);
    }

    // This deliberately exceeds the public allowance; validation precedes I/O.
    for cap in [0, TableSnapshotLimits::MAX_ROWS + 1] {
        let mut source = fixture::Sparse::new(6, true);
        let result = read_table_index_with_request(
            &mut source,
            CatalogScope::new(1),
            "Items",
            "Compound",
            complete_request(cap),
            &mut budget(),
            &|| false,
        );
        assert_eq!(result.err(), Some(InspectError::InvalidSnapshotLimits));
        assert_eq!(source.bytes_read, 0);
        assert_eq!(source.data_reads, 0);
    }
}
