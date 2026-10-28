use super::*;
use tablecraft_core::limits::Resource;
use tablecraft_query::{QueryLimits, prepare};

fn sparse_matches() -> Data {
    let mut data = Data::new();
    data.left = (0..80)
        .map(|id| vec![Value::Int32(id), Value::Int32(id)])
        .collect();
    data.right = (100..180)
        .map(|id| vec![Value::Int32(id), Value::Int32(id)])
        .collect();
    data.right[79][0] = Value::Int32(0);
    data.left_lists = (0..80)
        .map(|id| vec![Value::Null, Value::Int32(id), Value::Int32(id)])
        .collect();
    data.right_lists = (100..180).map(|id| vec![Value::Int32(id)]).collect();
    data
}

#[test]
fn scalar_equality_avoids_discarded_pair_allocations_and_keeps_complete_lists() {
    let data = sparse_matches();
    let options = ExecutionOptions {
        limits: QueryLimits {
            bytes: 512 * 1024,
            ..Default::default()
        },
        ..Default::default()
    };
    let syntax = parse_query(
        "SELECT L.Tags,R.Tags FROM LeftRows L INNER JOIN RightRows R ON L.Id=R.Id",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &data).unwrap();
    let result = plan.execute_collections(&data, &[], options).unwrap();
    assert_eq!(result.rows().len(), 1);
    assert_eq!(
        list(&result.rows()[0][0]),
        &[Value::Null, Value::Int32(0), Value::Int32(0)]
    );
    assert_eq!(list(&result.rows()[0][1]), &[Value::Int32(179)]);
    assert_eq!(*data.scans.borrow(), ["lists:LeftRows", "lists:RightRows"]);

    // A compound ON expression retains eager evaluation and the existing
    // allocated-pair path under the same deliberately bounded byte allowance.
    let compound = parse_query(
        "SELECT L.Tags,R.Tags FROM LeftRows L INNER JOIN RightRows R ON L.Id=R.Id AND L.Seq>=0",
        Default::default(),
    )
    .unwrap();
    let compound = prepare(compound.query(), &data).unwrap();
    assert!(matches!(
        compound.execute_collections(&data, &[], options).err(),
        Some(QueryError::Limit(error)) if error.resource == Resource::Bytes
    ));
}

#[test]
fn borrowed_null_keys_keep_unknown_and_outer_parent_absence_separate() {
    let mut data = Data::new();
    data.columns[0] = Column::new("Id".into(), ScalarType::Int32, true).unwrap();
    data.left[0][0] = Value::Null;
    data.right[2][0] = Value::Null;

    let left = execute(
        "SELECT L.Seq,L.Tags,R.Tags FROM LeftRows L LEFT JOIN RightRows R ON L.Id=R.Id",
        &data,
    )
    .unwrap();
    assert_eq!(left.rows().len(), 4);
    assert_eq!(integer(&left.rows()[0][0]), 0);
    assert_eq!(list(&left.rows()[0][1]), data.left_lists[0]);
    assert!(matches!(left.rows()[0][2], QueryValue::AbsentMultivalue));
    assert_eq!(list(&left.rows()[1][1]), &[]);
    assert_eq!(list(&left.rows()[1][2]), data.right_lists[0]);
    assert_eq!(list(&left.rows()[2][2]), data.right_lists[1]);
    assert_eq!(integer(&left.rows()[3][0]), 2);
    assert!(matches!(left.rows()[3][2], QueryValue::AbsentMultivalue));

    let right = execute(
        "SELECT L.Tags,R.Tags FROM LeftRows L RIGHT JOIN RightRows R ON R.Id=L.Id",
        &data,
    )
    .unwrap();
    assert_eq!(right.rows().len(), 3);
    assert!(matches!(right.rows()[2][0], QueryValue::AbsentMultivalue));
    assert_eq!(list(&right.rows()[2][1]), data.right_lists[2]);
}

#[test]
fn unmatched_lists_and_failed_empty_side_suffixes_are_still_validated() {
    let mut data = sparse_matches();
    data.right_lists[0].push(Value::Text("invalid unmatched child".into()));
    let sql = "SELECT L.Tags,R.Tags FROM LeftRows L INNER JOIN RightRows R ON L.Id=R.Id";
    assert_eq!(execute(sql, &data).err(), Some(QueryError::Source));

    data.right_lists[0].pop();
    data.left.clear();
    data.left_lists.clear();
    data.fail_right_eof = true;
    assert_eq!(execute(sql, &data).err(), Some(QueryError::Source));

    let data = Data::new();
    assert_eq!(
        execute(
            "SELECT L.Tags,R.Tags FROM LeftRows L INNER JOIN RightRows R ON L.Id=R.Id AND 1/(L.Id-L.Id)=0 WHERE False",
            &data,
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::DivisionByZero))
    );
}
