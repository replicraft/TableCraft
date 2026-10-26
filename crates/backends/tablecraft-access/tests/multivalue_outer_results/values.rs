use super::*;

#[test]
fn left_and_right_keep_empty_null_and_absent_lists_separate() {
    for code in [2, 5, 6] {
        let source = NativeSource::new(
            Cursor::new(join_fixture::with_null_parent(code)),
            CatalogScope::new(1),
            allowance(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();

        for (kind, order, pairs, nullable) in [
            (
                "LEFT",
                "a.Id",
                [(Some(1), None), (Some(2), Some(1)), (Some(3), Some(2))],
                [false, true],
            ),
            (
                "RIGHT",
                "b.Id",
                [(Some(2), Some(1)), (Some(3), Some(2)), (None, Some(3))],
                [true, false],
            ),
        ] {
            let sql = format!(
                "SELECT a.Id,b.Id,a.Score,b.Score FROM Items a \
                {kind} JOIN Items b ON a.Id=b.Id+1 ORDER BY {order}"
            );
            let parsed = parse_query(&sql, Default::default()).unwrap();

            let prepared = prepare(parsed.query(), &source).unwrap();
            for (output, nullable) in prepared.output_types().unwrap()[2..].iter().zip(nullable) {
                let OutputType::Multivalue {
                    element_type,
                    element_nullable,
                    parent_nullable,
                } = output
                else {
                    panic!("Outer collection lost its typed descriptor");
                };
                assert_eq!(
                    element_type.scalar_type(),
                    tablecraft_schema::ScalarType::Int32
                );
                assert!(*element_nullable);
                assert_eq!(*parent_nullable, nullable);
            }
            let result = prepared
                .execute_collections(&source, &[], Default::default())
                .unwrap();

            assert_eq!(result.rows().len(), pairs.len());
            for (row, (left, right)) in result.rows().iter().zip(pairs) {
                assert_outer(row, left, right);
            }
        }
    }
}

#[test]
fn nested_outer_joins_preserve_absence_across_two_missing_sides() {
    let source = NativeSource::new(
        Cursor::new(join_fixture::with_null_parent(6)),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let rows = execute(
        &source,
        "SELECT a.Id,b.Id,c.Id,a.Score,b.Score,c.Score \
        FROM (Items a LEFT JOIN Items b ON a.Id=b.Id+1) \
        RIGHT JOIN Items c ON b.Id=c.Id+1 ORDER BY c.Id",
    )
    .unwrap();

    assert_eq!(rows.len(), 3);
    assert_eq!(scalar(&rows[0][0]), Value::Int32(3));
    assert_eq!(scalar(&rows[0][1]), Value::Int32(2));
    assert_eq!(scalar(&rows[0][2]), Value::Int32(1));
    assert_eq!(list(&rows[0][3]), vec![]);
    assert_eq!(list(&rows[0][4]), vec![Value::Null]);
    assert_eq!(list(&rows[0][5]), members(1, 0));
    for (row, id, wanted) in [(&rows[1], 2, vec![Value::Null]), (&rows[2], 3, vec![])] {
        assert_eq!(scalar(&row[0]), Value::Null);
        assert_eq!(scalar(&row[1]), Value::Null);
        assert_eq!(scalar(&row[2]), Value::Int32(id));
        absent(&row[3]);
        absent(&row[4]);
        assert_eq!(list(&row[5]), wanted);
    }
}

#[test]
fn hidden_member_criteria_on_nullable_side_keep_the_complete_matched_list() {
    let source = NativeSource::new(
        Cursor::new(join_fixture::with_null_parent(5)),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();

    let rows = execute(
        &source,
        "SELECT a.Id,b.Id,a.Score,b.Score FROM Items a \
        LEFT JOIN Items b ON a.Id=b.Id+1 \
        WHERE b.Score.Value=100 AND b.Score.Value=101",
    )
    .unwrap();

    assert_eq!(rows.len(), 1);
    assert_outer(&rows[0], Some(2), Some(1));
}
