use super::support::*;
use tablecraft_core::values::Value;
use tablecraft_query::{OutputType, prepare};

#[test]
fn same_origin_aliases_zip_by_child_position_without_comparing_payloads() {
    for declared in [false, true] {
        let data = Data::fixture();
        let mut expected = Vec::new();
        for parent in &data.tables[0].rows {
            for value in expanded(&parent.lists[0]) {
                expected.push(vec![
                    scalar(parent.id()),
                    list(parent, 0),
                    stored(value.clone()),
                    list(parent, 0),
                    stored(value),
                ]);
            }
        }
        let sql = "SELECT Q.Id,Q.X,Q.X.Value,Q.Y,Q.Y.Value FROM \
                   (SELECT Id,A AS X,A AS Y FROM L) Q";
        let result = execute(sql, &data, declared).unwrap();
        assert_rows(&result, &expected);
        assert_eq!(*data.scans.borrow(), ["L"]);
    }
}

#[test]
fn independently_bound_named_self_join_references_form_a_product_despite_equal_payloads() {
    let mut data = Data::fixture();
    data.definitions
        .push(("Bundle", syntax("SELECT Id,A AS X FROM L")));
    let parent = &data.tables[0].rows[0];
    let mut expected = Vec::new();
    for left in &parent.lists[0] {
        for right in &parent.lists[0] {
            expected.push(vec![
                stored(left.clone()),
                stored(right.clone()),
                list(parent, 0),
                list(parent, 0),
            ]);
        }
    }
    let sql = "SELECT P.X.Value,Q.X.Value,P.X,Q.X FROM Bundle P \
               INNER JOIN Bundle Q ON P.Id=Q.Id WHERE P.Id=1";
    let result = execute(sql, &data, false).unwrap();
    assert_rows(&result, &expected);
    assert_eq!(result.rows().len(), 9);
    assert_eq!(*data.scans.borrow(), ["L", "L"]);
}

#[test]
fn renamed_aliases_share_visible_alignment_but_separate_hidden_atoms_keep_membership_semantics() {
    let mut data = Data::fixture();
    data.definitions
        .push(("Origin", syntax("SELECT Id,A AS X,A AS Y FROM L")));
    data.definitions
        .push(("Renamed", syntax("SELECT Id,Y AS P,X AS Q FROM Origin")));
    let sql = "SELECT P.Value,Q.Value FROM Renamed WHERE Q.Value=7";
    let result = execute(sql, &data, false).unwrap();
    assert_rows(
        &result,
        &[vec![scalar(7), scalar(7)], vec![scalar(7), scalar(7)]],
    );

    let sql = "SELECT X,Y FROM Origin WHERE X.Value=7 AND Y.Value IS NULL";
    let result = execute(sql, &data, false).unwrap();
    assert_rows(
        &result,
        &[vec![
            list(&data.tables[0].rows[0], 0),
            list(&data.tables[0].rows[0], 0),
        ]],
    );

    let sql = "SELECT X FROM Origin WHERE NOT X.Value=7";
    let result = execute(sql, &data, false).unwrap();
    assert_rows(
        &result,
        &[
            vec![list(&data.tables[0].rows[1], 0)],
            vec![list(&data.tables[0].rows[3], 0)],
        ],
    );
}

#[test]
fn explicit_outer_expansion_does_not_reuse_an_inner_child_iterator() {
    let data = Data::fixture();
    let parent = &data.tables[0].rows[0];
    let mut expected = Vec::new();
    for old_child in &parent.lists[0] {
        for new_child in &parent.lists[0] {
            expected.push(vec![
                stored(old_child.clone()),
                stored(new_child.clone()),
                list(parent, 0),
            ]);
        }
    }
    let sql = "SELECT Q.Child,Q.Whole.Value,Q.Whole FROM \
               (SELECT A AS Whole,A.Value AS Child FROM L WHERE Id=1) Q";
    let result = execute(sql, &data, false).unwrap();
    assert_rows(&result, &expected);
    assert_eq!(result.rows().len(), 9);
    assert_eq!(*data.scans.borrow(), ["L"]);
}

#[test]
fn missing_parent_empty_list_and_null_child_survive_retained_member_access() {
    let mut data = Data::fixture();
    data.tables[1].rows.push(parent(2, &[], &[]));
    data.tables[1].rows.push(parent(3, &[None], &[]));
    let sql = "SELECT Q.Id,Q.Whole,Q.Whole.Value FROM \
               (SELECT P.Id,R.A AS Whole FROM L P LEFT JOIN R ON P.Id=R.Id) Q";
    let plan = prepare(&syntax(sql), &data).unwrap();
    assert!(data.scans.borrow().is_empty());
    assert!(matches!(
        plan.output_types().unwrap()[1],
        OutputType::Multivalue {
            parent_nullable: true,
            element_nullable: true,
            ..
        }
    ));
    assert!(matches!(
        plan.output_types().unwrap()[2],
        OutputType::Scalar { nullable: true, .. }
    ));
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    let right = &data.tables[1].rows;
    let expected = vec![
        vec![scalar(1), list(&right[0], 0), scalar(10)],
        vec![scalar(1), list(&right[0], 0), scalar(11)],
        vec![scalar(2), list(&right[2], 0), stored(Value::Null)],
        vec![scalar(3), list(&right[3], 0), stored(Value::Null)],
        vec![scalar(4), Expected::Absent, stored(Value::Null)],
    ];
    assert_rows(&result, &expected);
}
