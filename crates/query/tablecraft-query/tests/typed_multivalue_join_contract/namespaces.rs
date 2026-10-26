use super::support::*;
use tablecraft_query::{QueryError, prepare};

fn source<'a>(value: i32, label: &str) -> Data<'a> {
    let mut data = Data::new(vec![Table::new(
        "T",
        vec![Parent::new(1, 0, &[Some(value)], label)],
    )]);
    data.definitions
        .push(("Owned", syntax("SELECT Id,A,B FROM T")));
    data
}

#[test]
fn equal_names_in_two_external_owners_and_explicit_root_registry_keep_exact_lists() {
    let bridge = source(701, "bridge");
    let mut left = source(501, "left");
    left.definitions
        .push(("Pinned", syntax("SELECT Id,A,B FROM [bridge].T")));
    let right = source(601, "right");
    let mut root = source(999, "root");
    root.external = vec![
        ("left", 1, &left),
        ("right", 2, &right),
        ("bridge", 3, &bridge),
    ];
    for (name, selected) in [("Owned", &left), ("Pinned", &bridge)] {
        let sql =
            format!("SELECT L.A,R.B FROM [left].{name} L INNER JOIN [right].Owned R ON L.Id=R.Id");
        let result = execute(&sql, &root, true).unwrap();
        assert_rows(
            &result,
            &[vec![
                list(&selected.tables[0].rows[0], 0),
                list(&right.tables[0].rows[0], 1),
            ]],
        );
    }
    assert!(root.scans.borrow().is_empty());
    assert!(
        left.external_calls.borrow().is_empty(),
        "explicit selectors inside the left definition use the operation root"
    );
    assert!(right.external_calls.borrow().is_empty());
    assert!(
        root.external_calls
            .borrow()
            .iter()
            .any(|token| token == "bridge")
    );
}

#[test]
fn external_rebinding_and_later_leaf_metadata_drift_refuse_before_any_cursor() {
    let left = source(501, "left");
    let right = source(601, "right");
    let mut root = source(999, "root");
    root.external = vec![("left", 1, &left), ("right", 2, &right)];
    let query = syntax("SELECT L.A,R.A FROM [left].T L INNER JOIN [right].T R ON L.Id=R.Id");
    let plan = prepare(&query, &root).unwrap();
    root.remap.set(true);
    assert_eq!(
        plan.execute_collections(&root, &[], Default::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    assert!(left.scans.borrow().is_empty());
    assert!(right.scans.borrow().is_empty());

    root.remap.set(false);
    right.changed.set(true);
    assert_eq!(
        plan.execute_collections(&root, &[], Default::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    assert!(left.scans.borrow().is_empty());
    assert!(right.scans.borrow().is_empty());
}
