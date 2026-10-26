use super::support::*;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{OutputType, QueryError, QueryValue, prepare};

#[test]
fn every_displayed_child_repeats_the_original_complete_list() {
    for declared in [false, true] {
        let data = Data::fixture();
        let mut expected = Vec::new();
        for row in &data.tables[0].rows {
            for value in expanded(&row.lists[0]) {
                expected.push(vec![
                    scalar(row.id()),
                    list(row, 0),
                    stored(value),
                    list(row, 0),
                ]);
            }
        }
        let sql = "SELECT Id,A,A.Value AS One,A AS Again FROM L";
        let result = execute(sql, &data, declared).unwrap();

        assert_rows(&result, &expected);
        assert_eq!(*data.scans.borrow(), ["L"]);
        assert_eq!(data.scalar_scans.get(), 0);
    }
}

#[test]
fn same_displayed_member_filters_rows_while_independent_hidden_member_filters_parents() {
    let data = Data::fixture();
    let parent = &data.tables[0].rows[0];
    let whole = &parent.lists[0];
    let sql = "SELECT A,A.Value AS One FROM L WHERE A.Value=7";
    let result = execute(sql, &data, false).unwrap();
    let expected = whole
        .iter()
        .filter(|value| **value == Value::Int32(7))
        .map(|value| vec![list(parent, 0), stored(value.clone())])
        .collect::<Vec<_>>();
    assert_rows(&result, &expected);

    let sql = "SELECT A,A.Value AS One FROM L WHERE B.Value=3";
    let result = execute(sql, &data, false).unwrap();
    let expected = whole
        .iter()
        .map(|value| vec![list(parent, 0), stored(value.clone())])
        .collect::<Vec<_>>();
    assert_rows(&result, &expected);

    let sql = "SELECT A,A.Value AS One FROM L WHERE NOT A.Value=7";
    let result = execute(sql, &data, false).unwrap();
    assert_rows(
        &result,
        &[vec![list(&data.tables[0].rows[3], 0), scalar(9)]],
    );
}

#[test]
fn independent_displayed_collections_form_products_without_trimming_either_list() {
    let data = Data::fixture();
    let mut expected = Vec::new();
    for parent in &data.tables[0].rows {
        for a in expanded(&parent.lists[0]) {
            for b in expanded(&parent.lists[1]) {
                expected.push(vec![
                    scalar(parent.id()),
                    list(parent, 1),
                    stored(a.clone()),
                    list(parent, 0),
                    stored(b),
                ]);
            }
        }
    }
    let sql = "SELECT Id,B,A.Value,A,B.Value FROM L";
    let result = execute(sql, &data, false).unwrap();

    assert_rows(&result, &expected);
}

#[test]
fn outer_matches_expand_before_joining_and_missing_parents_remain_absent() {
    for kind in ["LEFT", "RIGHT"] {
        let data = Data::fixture();
        let mut expected = Vec::new();
        let mut matched_right = vec![false; data.tables[1].rows.len()];
        for left in &data.tables[0].rows {
            for a in expanded(&left.lists[0]) {
                let mut matched = false;
                for (position, right) in data.tables[1].rows.iter().enumerate() {
                    if left.id() == right.id() {
                        matched = true;
                        matched_right[position] = true;
                        for b in expanded(&right.lists[0]) {
                            expected.push(vec![
                                list(left, 0),
                                stored(a.clone()),
                                list(right, 0),
                                stored(b),
                            ]);
                        }
                    }
                }
                if !matched && kind == "LEFT" {
                    expected.push(vec![
                        list(left, 0),
                        stored(a),
                        Expected::Absent,
                        stored(Value::Null),
                    ]);
                }
            }
        }
        if kind == "RIGHT" {
            for (position, right) in data.tables[1].rows.iter().enumerate() {
                if !matched_right[position] {
                    for b in expanded(&right.lists[0]) {
                        expected.push(vec![
                            Expected::Absent,
                            stored(Value::Null),
                            list(right, 0),
                            stored(b),
                        ]);
                    }
                }
            }
        }
        let sql = format!("SELECT L.A,L.A.Value,R.A,R.A.Value FROM L {kind} JOIN R ON L.Id=R.Id");
        let result = execute(&sql, &data, false).unwrap();
        assert_rows(&result, &expected);
    }
}

#[test]
fn named_derived_aliases_sort_and_top_the_expanded_rows_without_expanding_again() {
    let mut data = Data::fixture();
    data.definitions.push((
        "ExpandedRows",
        syntax("SELECT Id,A AS Whole,A.Value AS One FROM L"),
    ));
    let sql = "SELECT TOP 2 Q.Whole,Q.One,Q.Whole AS Again FROM \
               (SELECT Id,Whole,One FROM ExpandedRows) Q ORDER BY Q.One DESC,Q.Id";
    let plan = prepare(&syntax(sql), &data).unwrap();
    assert!(data.scans.borrow().is_empty());
    assert!(matches!(
        plan.output_types().unwrap(),
        [
            OutputType::Multivalue {
                parent_nullable: false,
                element_nullable: true,
                ..
            },
            OutputType::Scalar { nullable: true, .. },
            OutputType::Multivalue {
                parent_nullable: false,
                ..
            }
        ]
    ));
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    let expected = vec![
        vec![
            list(&data.tables[0].rows[3], 0),
            scalar(9),
            list(&data.tables[0].rows[3], 0),
        ],
        vec![
            list(&data.tables[0].rows[0], 0),
            scalar(7),
            list(&data.tables[0].rows[0], 0),
        ],
        // Both 7 rows also share Id=1, so the complete ORDER BY key ties.
        vec![
            list(&data.tables[0].rows[0], 0),
            scalar(7),
            list(&data.tables[0].rows[0], 0),
        ],
    ];
    assert_rows(&result, &expected);
    assert_eq!(*data.scans.borrow(), ["L"]);
}

#[test]
fn whole_single_lists_keep_every_ieee_bit_when_an_independent_member_is_displayed() {
    let bits = [
        0x00000000, 0x80000000, 0x00000001, 0x7f800000, 0x7fc00001, 0xffc12345,
    ];
    for declared in [false, true] {
        let data = single_fixture(&bits);
        let sql = "SELECT Q.Whole,Q.One FROM \
                   (SELECT A AS Whole,B.Value AS One FROM L) Q";
        let result = execute(sql, &data, declared).unwrap();
        assert_eq!(result.rows().len(), 2);
        for (row, expected_member) in result.rows().iter().zip([20, 21]) {
            let QueryValue::Multivalue(whole) = &row[0] else {
                panic!("the complete Single list must remain present");
            };
            let actual_bits = whole
                .values()
                .iter()
                .map(|value| {
                    let Value::Single(value) = value else {
                        panic!("a stored Single child changed carrier");
                    };
                    value.to_bits()
                })
                .collect::<Vec<_>>();
            assert_eq!(actual_bits, bits);
            assert_eq!(
                row[1],
                QueryValue::Scalar(tablecraft_core::values::ExpressionValue::Value(
                    Value::Int32(expected_member)
                ))
            );
        }

        let sql = "SELECT A,A.Value,B.Value FROM L";
        assert_eq!(
            execute(sql, &data, declared).err(),
            Some(QueryError::Evaluation(EvalError::NonFinite))
        );
    }
}
