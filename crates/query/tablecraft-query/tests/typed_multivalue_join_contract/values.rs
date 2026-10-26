use super::support::*;

#[test]
fn equal_join_keys_keep_duplicate_parents_and_both_physical_collection_zeroes() {
    for declared in [false, true] {
        for on in ["L.Id=R.Id", "R.Id=L.Id"] {
            let data = Data::fixture();
            let mut expected = Vec::new();
            for left in &data.tables[0].rows {
                for right in &data.tables[1].rows {
                    if left.id() == right.id() {
                        expected.push(vec![
                            list(left, 0),
                            list(right, 0),
                            scalar(left.id()),
                            scalar(right.key()),
                        ]);
                    }
                }
            }
            let sql = format!("SELECT L.A,R.A,L.Id,R.K FROM L INNER JOIN R ON {on}");
            let result = execute(&sql, &data, declared).unwrap();
            assert_rows(&result, &expected);
            assert_eq!(*data.scans.borrow(), ["L", "R"]);
            assert_eq!(data.scalar_scans.get(), 0);

            let data = Data::fixture();
            let mut expected = Vec::new();
            for left in &data.tables[0].rows {
                for right in &data.tables[1].rows {
                    if left.id() == right.id() {
                        expected.push(vec![
                            scalar(left.id()),
                            scalar(left.key()),
                            list(left, 1),
                            list(left, 0),
                            scalar(right.id()),
                            scalar(right.key()),
                            list(right, 1),
                            list(right, 0),
                        ]);
                    }
                }
            }
            let result = execute(
                "SELECT L.*,R.* FROM L INNER JOIN R ON L.Id=R.Id",
                &data,
                declared,
            )
            .unwrap();
            assert_rows(&result, &expected);
        }
    }
}

#[test]
fn nested_right_offsets_match_a_linear_three_parent_model() {
    for sql in [
        "SELECT L.A,R.B,S.A,L.Id FROM L INNER JOIN (R INNER JOIN S ON R.K=S.Id) ON L.Id=R.Id",
        "SELECT L.A,R.B,S.A,L.Id FROM L,R,S WHERE L.Id=R.Id AND R.K=S.Id",
    ] {
        let data = Data::fixture();
        let mut expected = Vec::new();
        for left in &data.tables[0].rows {
            for right in &data.tables[1].rows {
                for third in &data.tables[2].rows {
                    if left.id() == right.id() && right.key() == third.id() {
                        expected.push(vec![
                            list(left, 0),
                            list(right, 1),
                            list(third, 0),
                            scalar(left.id()),
                        ]);
                    }
                }
            }
        }
        let result = execute(sql, &data, false).unwrap();
        assert_rows(&result, &expected);
        assert_eq!(*data.scans.borrow(), ["L", "R", "S"]);
    }
}

#[test]
fn named_derived_lists_keep_dense_aliases_and_scalar_only_leaves_use_scalar_scans() {
    let mut data = Data::fixture();
    data.definitions
        .push(("ProjectedRows", syntax("SELECT A AS Items,Id,K FROM R")));
    let mut pairs = Vec::new();
    for left in &data.tables[0].rows {
        for right in &data.tables[1].rows {
            if left.id() == right.id() {
                pairs.push((left.id() + right.key(), left, right));
            }
        }
    }
    pairs.sort_by_key(|(number, _, _)| *number);
    let expected = pairs
        .iter()
        .map(|(number, left, right)| vec![list(left, 1), list(right, 0), scalar(*number)])
        .collect::<Vec<_>>();
    let result = execute(
        "SELECT L.B,V.Items,L.Id+V.K AS N FROM L INNER JOIN ProjectedRows V ON L.Id=V.Id ORDER BY N",
        &data,
        true,
    )
    .unwrap();
    assert_rows(&result, &expected);

    let mut data = Data::fixture();
    let mut plain = Table::new(
        "Plain",
        vec![
            Parent::new(1, 0, &[], "unused"),
            Parent::new(2, 0, &[], "unused"),
        ],
    );
    plain.plain = true;
    data.tables.push(plain);
    let expected = data.tables[0]
        .rows
        .iter()
        .map(|row| vec![list(row, 0), scalar(row.id())])
        .collect::<Vec<_>>();
    let result = execute(
        "SELECT L.A,P.Id FROM L INNER JOIN Plain P ON L.Id=P.Id",
        &data,
        false,
    )
    .unwrap();
    assert_rows(&result, &expected);
    assert_eq!(data.scalar_scans.get(), 1);

    // A scalar-only left leaf contributes zero collection positions, even
    // though its ordinary scalar frame is nonempty.
    let expected = data.tables[0]
        .rows
        .iter()
        .map(|row| vec![scalar(row.id()), list(row, 0)])
        .collect::<Vec<_>>();
    let result = execute(
        "SELECT P.Id,L.A FROM Plain P INNER JOIN L ON P.Id=L.Id",
        &data,
        false,
    )
    .unwrap();
    assert_rows(&result, &expected);
    assert_eq!(data.scalar_scans.get(), 2);
}

#[test]
fn hidden_criteria_choose_parent_pairs_without_trimming_or_repeating_display_lists() {
    let data = Data::fixture();
    let mut expected = Vec::new();
    for left in &data.tables[0].rows {
        for right in &data.tables[1].rows {
            let left_contains = left.lists[0].contains(&tablecraft_core::values::Value::Int32(10));
            let right_contains =
                right.lists[0].contains(&tablecraft_core::values::Value::Int32(101));
            if left.id() == right.id() && left_contains && right_contains {
                expected.push(vec![list(left, 0), list(right, 0), list(left, 0)]);
            }
        }
    }
    let sql = "SELECT L.A,R.A,L.A AS Again \
               FROM L INNER JOIN R ON L.Id=R.Id \
               WHERE L.A.Value=10 AND R.A.Value=101";
    let result = execute(sql, &data, false).unwrap();

    assert_rows(&result, &expected);
}
