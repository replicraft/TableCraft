use super::*;

#[test]
fn native_mixed_rows_repeat_complete_payloads_without_replacing_empty_or_null_lists() {
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
            let sql = "SELECT Id,Score,Score.Value AS One,Score AS Again FROM Items";
            let rows = execute(&source, sql, declared);
            let mut expected = Vec::new();
            for id in 1..=3 {
                let whole = members(id, true, 0);
                let values = if whole.is_empty() {
                    vec![Value::Null]
                } else {
                    whole.clone()
                };
                for value in values {
                    expected.push((id, whole.clone(), value));
                }
            }
            assert_eq!(rows.len(), expected.len());
            for (row, (id, whole, value)) in rows.iter().zip(expected) {
                assert_eq!(scalar(&row[0]), &Value::Int32(id));
                assert_eq!(list(&row[1]), whole);
                assert_eq!(scalar(&row[2]), &value);
                assert_eq!(list(&row[3]), whole);
            }
        }
    }
}

#[test]
fn displayed_member_filters_and_independent_joined_parent_criteria_keep_different_scopes() {
    let source = NativeSource::new(
        Cursor::new(join_fixture::database(6, 0)),
        CatalogScope::new(1),
        allowance(),
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let sql = "SELECT Score,Score.Value AS One FROM Items WHERE Score.Value=101";
    let rows = execute(&source, sql, false);
    assert_eq!(rows.len(), 2);
    for row in &rows {
        assert_eq!(list(&row[0]), members(1, false, 0));
        assert_eq!(scalar(&row[1]), &Value::Int32(101));
    }

    let sql = "SELECT a.Score,a.Score.Value AS One,b.Score FROM Items a \
               INNER JOIN Items b ON a.Id<b.Id WHERE b.Score.Value=200";
    let rows = execute(&source, sql, false);
    assert_eq!(rows.len(), 4);
    for (row, value) in rows.iter().zip(members(1, false, 0)) {
        assert_eq!(list(&row[0]), members(1, false, 0));
        assert_eq!(scalar(&row[1]), &value);
        assert_eq!(list(&row[2]), members(2, false, 0));
    }
}

#[test]
fn native_outer_mixed_rows_transport_absence_and_complete_lists_through_a_derived_layer() {
    for code in [2, 5, 6] {
        let source = NativeSource::new(
            Cursor::new(join_fixture::with_null_parent(code)),
            CatalogScope::new(1),
            allowance(),
            &|| false,
        )
        .unwrap()
        .with_multivalues();
        let sql = "SELECT Q.Id,Q.Whole,Q.One FROM \
                   (SELECT a.Id,b.Score AS Whole,b.Score.Value AS One FROM Items a \
                    LEFT JOIN Items b ON a.Id=b.Id+1) Q";
        let rows = execute(&source, sql, false);
        assert_eq!(rows.len(), 6);
        assert_eq!(scalar(&rows[0][0]), &Value::Int32(1));
        assert!(matches!(rows[0][1], QueryValue::AbsentMultivalue));
        assert_eq!(scalar(&rows[0][2]), &Value::Null);
        for (row, value) in rows[1..5].iter().zip(members(1, true, 0)) {
            assert_eq!(scalar(&row[0]), &Value::Int32(2));
            assert_eq!(list(&row[1]), members(1, true, 0));
            assert_eq!(scalar(&row[2]), &value);
        }
        assert_eq!(scalar(&rows[5][0]), &Value::Int32(3));
        assert_eq!(list(&rows[5][1]), [Value::Null]);
        assert_eq!(scalar(&rows[5][2]), &Value::Null);
    }
}

#[test]
fn explicit_external_leaf_keeps_its_own_whole_and_displayed_values() {
    let pool = std::cell::RefCell::new(allowance());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(join_fixture::database(6, 0)),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(join_fixture::database(6, 1000)),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_multivalues();
    let mappings = [NativeExternalMapping::new(
        "remote",
        tablecraft_query::source::SourceKey::new(7),
        target.handle(),
    )];
    let source = ExternalNativeSource::new(root.handle(), &mappings).unwrap();
    let sql = "SELECT a.Score,b.Score,b.Score.Value AS One FROM Items a \
               INNER JOIN [remote].Items b ON a.Id=b.Id WHERE a.Id=1";
    let rows = execute(&source, sql, false);
    assert_eq!(rows.len(), 4);
    for (row, value) in rows.iter().zip(members(1, false, 1000)) {
        assert_eq!(list(&row[0]), members(1, false, 0));
        assert_eq!(list(&row[1]), members(1, false, 1000));
        assert_eq!(scalar(&row[2]), &value);
    }
}
