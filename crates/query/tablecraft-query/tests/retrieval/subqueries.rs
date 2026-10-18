use super::*;

#[test]
fn subqueries_derived_sources_and_correlated_scopes() {
    let source = source();
    let r=run("SELECT i.ID FROM Items AS i WHERE EXISTS (SELECT c.ID FROM Categories AS c WHERE c.ID=i.Category) ORDER BY i.ID",&source).unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(1)], &[Some(2)]]));
    let r = run(
        "SELECT d.ID FROM (SELECT ID FROM Items WHERE Price=8) AS d ORDER BY d.ID DESC",
        &source,
    )
    .unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(4)], &[Some(2)]]));
    let r=run("SELECT (SELECT Count(*) FROM Items AS i WHERE i.Category=c.ID) AS N FROM Categories AS c ORDER BY c.ID",&source).unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(2)], &[Some(0)]]));
    assert_eq!(
        run("SELECT (SELECT ID FROM Empty)", &source)
            .unwrap()
            .rows(),
        ints(&[&[None]])
    );
    assert_eq!(
        run("SELECT (SELECT ID FROM Items)", &source).err(),
        Some(QueryError::Cardinality)
    );
    assert_eq!(
        run(
            "SELECT ID FROM Items WHERE Category IN (SELECT ID FROM Categories) ORDER BY ID",
            &source
        )
        .unwrap()
        .rows(),
        ints(&[&[Some(1)], &[Some(2)]])
    );
}
#[test]
fn membership_quantifiers_and_reversed_between_bounds() {
    let source = source();
    let r = run(
        "SELECT ID FROM Items WHERE ID BETWEEN 3 AND 1 AND ID IN (1, 3, NULL) ORDER BY ID",
        &source,
    )
    .unwrap();
    assert_eq!(r.rows(), ints(&[&[Some(1)], &[Some(3)]]));
    let r=run("SELECT NULL IN (SELECT ID FROM Empty) AS A, NULL > ALL (SELECT ID FROM Empty) AS B, 1 = ANY (SELECT ID FROM Empty) AS C",&source).unwrap();
    assert_eq!(
        r.rows(),
        &[vec![
            Value::Boolean(false),
            Value::Boolean(true),
            Value::Boolean(false)
        ]]
    );
    let r = run(
        "SELECT 8 NOT IN (SELECT Price FROM Items) AS A, 9 NOT IN (SELECT Price FROM Items) AS B",
        &source,
    )
    .unwrap();
    assert_eq!(r.rows(), &[vec![Value::Boolean(false), Value::Null]]);
}
