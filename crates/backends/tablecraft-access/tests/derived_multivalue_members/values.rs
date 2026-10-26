use super::*;

#[test]
fn derived_alias_members_preserve_native_lists_duplicates_nulls_and_empty_placeholders() {
    for code in [2, 5, 6] {
        for declared in [false, true] {
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
                "SELECT Q.Id,Q.Whole,Q.Whole.Value AS One FROM \
                 (SELECT Id,Score AS Whole FROM Items) Q",
                declared,
            );
            let mut expected = Vec::new();
            for id in 1..=3 {
                let whole = members(id, true, 0);
                let selected = if whole.is_empty() {
                    vec![Value::Null]
                } else {
                    whole.clone()
                };
                for one in selected {
                    expected.push((id, whole.clone(), one));
                }
            }
            assert_eq!(rows.len(), expected.len());
            for (row, (id, whole, one)) in rows.iter().zip(expected) {
                assert_eq!(scalar(&row[0]), &Value::Int32(id));
                assert_eq!(list(&row[1]), whole);
                assert_eq!(scalar(&row[2]), &one);
            }
        }
    }
}

#[test]
fn named_repeated_aliases_align_members_without_forming_a_child_cartesian_product() {
    let source = NativeSource::new(
        Cursor::new(join_fixture::database(6, 0)),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let syntax = parse_query(
        "SELECT Id,Score AS FirstList,Score AS OtherList FROM Items",
        Default::default(),
    )
    .unwrap();
    let named = Named {
        source: &source,
        definition: syntax.query().clone(),
    };
    let rows = execute(
        &named,
        "SELECT Q.FirstList,Q.FirstList.Value AS A,Q.OtherList.Value AS B \
         FROM Retained Q WHERE Q.Id=1",
        false,
    );
    assert_eq!(rows.len(), 4);
    for (row, one) in rows.iter().zip(members(1, false, 0)) {
        assert_eq!(list(&row[0]), members(1, false, 0));
        assert_eq!(scalar(&row[1]), &one);
        assert_eq!(scalar(&row[2]), &one);
    }
}

#[test]
fn derived_outer_parent_absence_remains_distinct_from_present_null_and_empty_lists() {
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
        "SELECT Q.Id,Q.Whole,Q.Whole.Value AS One FROM \
         (SELECT a.Id,b.Score AS Whole FROM Items a \
          LEFT JOIN Items b ON a.Id=b.Id+1) Q",
        false,
    );
    assert_eq!(rows.len(), 6);
    assert_eq!(scalar(&rows[0][0]), &Value::Int32(1));
    assert!(matches!(rows[0][1], QueryValue::AbsentMultivalue));
    assert_eq!(scalar(&rows[0][2]), &Value::Null);
    for (row, one) in rows[1..5].iter().zip(members(1, true, 0)) {
        assert_eq!(scalar(&row[0]), &Value::Int32(2));
        assert_eq!(list(&row[1]), members(1, true, 0));
        assert_eq!(scalar(&row[2]), &one);
    }
    assert_eq!(scalar(&rows[5][0]), &Value::Int32(3));
    assert_eq!(list(&rows[5][1]), [Value::Null]);
    assert_eq!(scalar(&rows[5][2]), &Value::Null);

    let empty = execute(
        &source,
        "SELECT Q.Whole,Q.Whole.Value FROM (SELECT Score AS Whole FROM Items WHERE Id=3) Q",
        false,
    );
    assert_eq!(empty.len(), 1);
    assert!(list(&empty[0][0]).is_empty());
    assert_eq!(scalar(&empty[0][1]), &Value::Null);
}
