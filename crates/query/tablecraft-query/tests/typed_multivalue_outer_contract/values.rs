use super::support::*;
use tablecraft_core::values::Value;
use tablecraft_query::{OutputType, QueryError, prepare};

fn absent() -> Expected {
    Expected::Absent
}

fn null() -> Expected {
    Expected::Scalar(Value::Null)
}

#[test]
fn left_and_right_keep_parent_absence_separate_from_empty_and_null_children() {
    for declared in [false, true] {
        for kind in ["LEFT", "RIGHT"] {
            let mut data = Data::fixture();
            data.tables[1].rows[2].lists[0] = vec![Value::Null];
            let left = &data.tables[0].rows;
            let right = &data.tables[1].rows;
            let mut expected = Vec::new();
            let mut matched_right = vec![false; right.len()];
            for l in left {
                let mut matched = false;
                for (position, r) in right.iter().enumerate() {
                    if l.id() == r.id() {
                        matched = true;
                        matched_right[position] = true;
                        expected.push(vec![scalar(l.id()), scalar(r.id()), list(l, 0), list(r, 0)]);
                    }
                }
                if !matched && kind == "LEFT" {
                    expected.push(vec![scalar(l.id()), null(), list(l, 0), absent()]);
                }
            }
            if kind == "RIGHT" {
                for (position, r) in right.iter().enumerate() {
                    if !matched_right[position] {
                        expected.push(vec![null(), scalar(r.id()), absent(), list(r, 0)]);
                    }
                }
            }

            let sql = format!("SELECT L.Id,R.Id,L.A,R.A FROM L {kind} JOIN R ON L.Id=R.Id");
            let result = execute(&sql, &data, declared).unwrap();
            assert_rows(&result, &expected);
            assert_eq!(*data.scans.borrow(), ["L", "R"]);
            assert_eq!(data.scalar_scans.get(), 0);
        }
    }
}

#[test]
fn nested_missing_subtrees_keep_each_list_slot_absent() {
    let data = Data::fixture();
    let mut expected = Vec::new();
    for l in &data.tables[0].rows {
        let mut matched = false;
        for r in &data.tables[1].rows {
            if l.id() == r.id() {
                matched = true;
                expected.push(vec![list(l, 1), list(r, 0), absent(), list(r, 0)]);
            }
        }
        if !matched {
            expected.push(vec![list(l, 1), absent(), absent(), absent()]);
        }
    }
    let sql = "SELECT L.B,R.A,S.B,R.A AS Again FROM L \
               LEFT JOIN (R LEFT JOIN S ON R.Id=S.Id) ON L.Id=R.Id";
    let result = execute(sql, &data, false).unwrap();

    assert_rows(&result, &expected);
    assert_eq!(*data.scans.borrow(), ["L", "R", "S"]);
}

#[test]
fn wildcard_and_named_derived_boundaries_preserve_presence_and_metadata() {
    let mut data = Data::fixture();
    data.definitions.push((
        "ProjectedRows",
        syntax("SELECT L.Id,L.A AS Kept,R.B AS Maybe FROM L LEFT JOIN R ON L.Id=R.Id"),
    ));
    let sql = "SELECT Q.Maybe,Q.Kept,Q.Maybe AS Again FROM \
               (SELECT Id,Kept,Maybe FROM ProjectedRows) Q";
    let plan = prepare(&syntax(sql), &data).unwrap();
    let types = plan.output_types().unwrap();
    assert!(matches!(
        types[0],
        OutputType::Multivalue {
            element_nullable: false,
            parent_nullable: true,
            ..
        }
    ));
    assert!(matches!(
        types[1],
        OutputType::Multivalue {
            element_nullable: true,
            parent_nullable: false,
            ..
        }
    ));
    let expected = vec![
        vec![
            list(&data.tables[1].rows[0], 1),
            list(&data.tables[0].rows[0], 0),
            list(&data.tables[1].rows[0], 1),
        ],
        vec![
            list(&data.tables[1].rows[1], 1),
            list(&data.tables[0].rows[0], 0),
            list(&data.tables[1].rows[1], 1),
        ],
        vec![
            list(&data.tables[1].rows[0], 1),
            list(&data.tables[0].rows[1], 0),
            list(&data.tables[1].rows[0], 1),
        ],
        vec![
            list(&data.tables[1].rows[1], 1),
            list(&data.tables[0].rows[1], 0),
            list(&data.tables[1].rows[1], 1),
        ],
        vec![absent(), list(&data.tables[0].rows[2], 0), absent()],
    ];
    let result = plan
        .execute_collections(&data, &[], Default::default())
        .unwrap();
    assert_rows(&result, &expected);

    let result = execute(
        "SELECT L.*,R.* FROM L LEFT JOIN R ON L.Id=R.Id WHERE L.Id=2",
        &data,
        false,
    )
    .unwrap();
    assert_rows(
        &result,
        &[vec![
            scalar(2),
            scalar(7),
            list(&data.tables[0].rows[2], 1),
            list(&data.tables[0].rows[2], 0),
            null(),
            null(),
            absent(),
            absent(),
        ]],
    );
}

#[test]
fn hidden_membership_filters_do_not_confuse_absent_and_empty_parents() {
    for (predicate, pairs) in [
        (
            "R.A.Value=100",
            vec![(0_usize, Some(0_usize)), (1, Some(0))],
        ),
        (
            "NOT (R.A.Value=100)",
            vec![(0, Some(1)), (1, Some(1)), (3, Some(2))],
        ),
    ] {
        let mut data = Data::fixture();
        data.tables[0].rows.push(Parent::new(3, 0, &[None], "L3"));
        let mut expected = Vec::new();
        for (left, right) in pairs {
            let right =
                right.map_or_else(absent, |position| list(&data.tables[1].rows[position], 0));
            expected.push(vec![list(&data.tables[0].rows[left], 0), right]);
        }
        let sql = format!("SELECT L.A,R.A FROM L LEFT JOIN R ON L.Id=R.Id WHERE {predicate}");
        let result = execute(&sql, &data, false).unwrap();
        assert_rows(&result, &expected);
    }

    // Presence tests belong to named collections. Bare scalar multivalue
    // outputs do not acquire that separate predicate capability through a join.
    for predicate in ["R.A IS NULL", "R.A IS NOT NULL"] {
        let data = Data::fixture();
        let sql = format!("SELECT L.A,R.A FROM L LEFT JOIN R ON L.Id=R.Id WHERE {predicate}");
        assert_eq!(
            execute(&sql, &data, false).err(),
            Some(QueryError::Unsupported)
        );
        assert!(data.scans.borrow().is_empty());
    }
}
