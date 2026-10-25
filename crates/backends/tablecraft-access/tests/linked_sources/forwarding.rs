use super::*;
use std::ops::Bound;
use tablecraft_query::source::ScanConstraint;

#[test]
fn projected_reads_skip_unselected_bad_carriers_and_keep_constraint_hints() {
    let mut bytes = target_fixture::database();
    bytes[10 * 4096..11 * 4096].copy_from_slice(&target_fixture::definition(
        &[("Id", 4, 0, 4), ("BadDate", 8, 4, 8), ("Score", 4, 12, 4)],
        3,
        11 << 8,
    ));
    let rows = (1..=3_i32)
        .map(|id| {
            let mut row = 3_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(f64::NAN.to_le_bytes());
            row.extend((id * 10).to_le_bytes());
            row.push(7);
            (0, row)
        })
        .collect::<Vec<_>>();
    bytes[12 * 4096..13 * 4096].copy_from_slice(&support::data_page(10, &rows));
    let pool = RefCell::new(budget());
    let root = NativeSource::new_with_shared_budget(
        Cursor::new(root_bytes()),
        CatalogScope::new(1),
        &pool,
        &|| false,
    )
    .unwrap();
    let target = NativeSource::new_with_shared_budget(
        Cursor::new(bytes),
        CatalogScope::new(2),
        &pool,
        &|| false,
    )
    .unwrap()
    .with_auto_indexes();
    let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
    assert!(facade.text_equality_hints("Linked").unwrap());
    let constraint = ScanConstraint::Int32 {
        column: 0,
        lower: Bound::Included(2),
        upper: Bound::Unbounded,
    };
    let mut scores = Vec::new();
    facade
        .scan_projected("Linked", &[2], Some(constraint), &mut |row| {
            scores.push(row.get(0).ok_or(QueryError::Source)?.clone());
            Ok(())
        })
        .unwrap();
    assert_eq!(
        scores,
        [Value::Int32(10), Value::Int32(20), Value::Int32(30)]
    );
    assert!(execute(&facade, "SELECT BadDate FROM Linked").is_err());
}

#[test]
fn multivalue_and_history_metadata_and_aligned_children_use_target_views() {
    for (bytes, sql, expected) in [
        (
            collection_fixture::populated(),
            "SELECT Id,Score.Value FROM Linked ORDER BY Id,Score.Value",
            vec![
                vec![Value::Int32(1), Value::Int32(100)],
                vec![Value::Int32(1), Value::Int32(101)],
                vec![Value::Int32(2), Value::Int32(200)],
                vec![Value::Int32(3), Value::Null],
            ],
        ),
        (
            history_fixture::database(),
            "SELECT Id FROM Linked WHERE Score IS NOT NULL ORDER BY Id",
            vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
        ),
    ] {
        let pool = RefCell::new(budget());
        let root = NativeSource::new_with_shared_budget(
            Cursor::new(root_bytes()),
            CatalogScope::new(1),
            &pool,
            &|| false,
        )
        .unwrap();
        let target = NativeSource::new_with_shared_budget(
            Cursor::new(bytes),
            CatalogScope::new(2),
            &pool,
            &|| false,
        )
        .unwrap()
        .with_collections();
        let facade = LinkedNativeSource::new(root.handle(), &mappings(&target)).unwrap();
        assert!(
            facade.unavailable_columns("Linked").unwrap()
                == target.unavailable_columns("Items").unwrap()
        );
        assert!(
            facade.multivalue_columns("Linked").unwrap()
                == target.multivalue_columns("Items").unwrap()
        );
        assert_eq!(
            facade.multivalue_text_collations("Linked").unwrap(),
            target.multivalue_text_collations("Items").unwrap()
        );
        assert!(
            facade.collection_members("Linked").unwrap()
                == target.collection_members("Items").unwrap()
        );
        assert_eq!(
            facade.collection_member_text_collations("Linked").unwrap(),
            target.collection_member_text_collations("Items").unwrap()
        );
        assert_eq!(execute(&facade, sql).unwrap(), expected);
    }
}
