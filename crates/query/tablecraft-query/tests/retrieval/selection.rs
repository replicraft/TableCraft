use super::*;

#[test]
fn projection_filter_order_and_hidden_sort_fields() {
    let source = source();
    let r=run("SELECT i.ID AS Code, i.Price * 2 AS Total FROM Items AS i WHERE i.Price > 4 ORDER BY Total DESC, Code DESC",&source).unwrap();
    assert_eq!(r.columns(), &["Code", "Total"]);
    assert_eq!(
        r.rows(),
        ints(&[&[Some(4), Some(16)], &[Some(2), Some(16)]])
    );
    let r = run("SELECT ID FROM Items ORDER BY Price, ID", &source).unwrap();
    assert_eq!(
        r.rows(),
        ints(&[&[Some(3)], &[Some(1)], &[Some(2)], &[Some(4)]])
    );
}
#[test]
fn joins_aliases_wildcards_and_unmatched_rows() {
    let source = source();
    let r=run("SELECT i.ID, c.* FROM Items AS i LEFT JOIN Categories AS c ON i.Category=c.ID ORDER BY i.ID",&source).unwrap();
    assert_eq!(
        r.rows(),
        ints(&[
            &[Some(1), Some(10)],
            &[Some(2), Some(10)],
            &[Some(3), None],
            &[Some(4), None]
        ])
    );
    let r=run("SELECT i.ID, c.ID AS CategoryID FROM Items AS i RIGHT JOIN Categories AS c ON i.Category=c.ID ORDER BY c.ID, i.ID",&source).unwrap();
    assert_eq!(
        r.rows(),
        ints(&[
            &[Some(1), Some(10)],
            &[Some(2), Some(10)],
            &[None, Some(30)]
        ])
    );
    assert_eq!(
        run("SELECT i.ID FROM Empty AS e, Items AS i", &source)
            .unwrap()
            .rows()
            .len(),
        0
    );
    assert_eq!(
        run("SELECT i.ID FROM Items AS i, Categories AS c", &source)
            .unwrap()
            .rows()
            .len(),
        8
    );
}
