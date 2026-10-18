use super::*;

#[test]
fn grouped_aggregates_having_null_keys_and_empty_global_group() {
    let source = source();
    let r=run("SELECT Category, Count(*) AS N, Count(Price) AS Known, Sum(Price) AS Total, Min(Price) AS Low, Max(Price) AS High FROM Items GROUP BY Category HAVING Count(*) > 1 ORDER BY Category",&source).unwrap();
    assert_eq!(
        r.rows(),
        ints(&[&[Some(10), Some(2), Some(2), Some(12), Some(4), Some(8)]])
    );
    let r = run(
        "SELECT Count(*) AS N, Sum(ID) AS Total, Avg(ID) AS Mean FROM Empty",
        &source,
    )
    .unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(0), None, None]]));
    let r = run(
        "SELECT Avg(Price) AS Mean FROM Items WHERE Category=10",
        &source,
    )
    .unwrap();
    assert_eq!(r.rows(), &[vec![Value::Double(Double::from_float(6.0))]]);
    let r = run(
        "SELECT Category, Count(*) AS N FROM Items GROUP BY Category ORDER BY Category",
        &source,
    )
    .unwrap();
    assert_eq!(
        r.rows(),
        ints(&[&[None, Some(1)], &[Some(10), Some(2)], &[Some(20), Some(1)]])
    );
    assert_eq!(
        run(
            "SELECT Category, ID, Count(*) FROM Items GROUP BY Category",
            &source
        )
        .err(),
        Some(QueryError::Grouping)
    );
}
#[test]
fn distinct_union_and_top_preserve_sql_null_equality_and_ties() {
    let source = source();
    let r = run(
        "SELECT DISTINCT Price FROM Items ORDER BY Price DESC",
        &source,
    )
    .unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(8)], &[Some(4)], &[None]]));
    let r = run("SELECT TOP 1 ID FROM Items ORDER BY Price DESC", &source).unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(2)], &[Some(4)]]));
    let r = run("SELECT TOP 25 PERCENT ID FROM Items ORDER BY ID", &source).unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(1)]]));
    let r = run(
        "SELECT Price AS P FROM Items UNION SELECT NULL UNION ALL SELECT 8 ORDER BY P DESC",
        &source,
    )
    .unwrap();
    assert_eq!(
        r.rows(),
        ints(&[&[Some(8)], &[Some(8)], &[Some(4)], &[None]])
    );
    let r = run("SELECT 2 AS N UNION SELECT 1 ORDER BY 1", &source).unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(1)], &[Some(2)]]));
}
