use super::*;

#[test]
fn self_join_remaps_same_typed_collection_zero_on_both_sides() {
    for code in [2, 5, 6] {
        let source = NativeSource::new(
            Cursor::new(fixture(code, 0)),
            CatalogScope::new(1),
            allowance(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();

        let sql = "SELECT a.Id AS A,b.Id AS B,a.Score AS L,b.Score AS R \
                   FROM Items a INNER JOIN Items b ON a.Id<b.Id ORDER BY a.Id,b.Id";
        let parsed = parse_query(sql, Default::default()).unwrap();

        let prepared = prepare(parsed.query(), &source).unwrap();
        for output in &prepared.output_types().unwrap()[2..] {
            let OutputType::Multivalue {
                element_type,
                element_nullable,
            } = output
            else {
                panic!("Joined list metadata became scalar/dynamic");
            };
            assert_eq!(
                element_type.scalar_type(),
                tablecraft_schema::ScalarType::Int32
            );
            assert!(*element_nullable);
        }
        let rows = prepared
            .execute_collections(&source, &[], Default::default())
            .unwrap();

        assert_eq!(rows.rows().len(), 3);
        for (row, (left, right)) in rows.rows().iter().zip([(1, 2), (1, 3), (2, 3)]) {
            assert_pair(row, left, right, 0);
        }
    }
}

#[test]
fn cross_join_cardinality_is_parents_not_the_product_of_children() {
    for code in [2, 5, 6] {
        let source = NativeSource::new(
            Cursor::new(fixture(code, 0)),
            CatalogScope::new(1),
            allowance(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();

        let rows = execute(
            &source,
            "SELECT a.Id,b.Id,a.Score,b.Score FROM Items a \
            ,Items b ORDER BY a.Id,b.Id",
        )
        .unwrap();

        assert_eq!(rows.len(), 9);
        for (position, row) in rows.iter().enumerate() {
            assert_pair(row, (position / 3 + 1) as i32, (position % 3 + 1) as i32, 0);
        }
    }
}

#[test]
fn joined_hidden_member_criteria_keep_both_complete_parent_lists() {
    let source = NativeSource::new(
        Cursor::new(fixture(6, 0)),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let rows = execute(
        &source,
        "SELECT a.Id,b.Id,a.Score,b.Score FROM Items a \
        INNER JOIN Items b ON a.Id<b.Id \
        WHERE a.Score.Value=100 AND a.Score.Value=101 AND b.Score.Value=200",
    )
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_pair(&rows[0], 1, 2, 0);
}

#[test]
fn joined_single_null_lists_remain_distinct_from_empty_parent_lists() {
    for code in [2, 5, 6] {
        let source = NativeSource::new(
            Cursor::new(join_fixture::with_null_parent(code)),
            CatalogScope::new(1),
            allowance(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();

        let rows = execute(
            &source,
            "SELECT a.Id,b.Id,a.Score,b.Score FROM Items a \
             INNER JOIN Items b ON a.Id=b.Id WHERE a.Id>=2 ORDER BY a.Id",
        )
        .unwrap();

        assert_eq!(rows.len(), 2);
        for (row, id, values) in [(&rows[0], 2, vec![Value::Null]), (&rows[1], 3, vec![])] {
            assert_eq!(scalar(&row[0]), Value::Int32(id));
            assert_eq!(scalar(&row[1]), Value::Int32(id));
            assert_eq!(list(&row[2]), values);
            assert_eq!(list(&row[3]), values);
        }
    }
}
