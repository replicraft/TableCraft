use super::support::*;

#[test]
fn domain_criteria_and_extrema_use_captured_field_policies_while_results_use_default() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let rows = run(
        "SELECT DCount('*','T',\"A='A'\"),DMin('B','T'),DMax('B','T')",
        &source,
        Some(&resolver),
    )
    .unwrap();
    assert_eq!(
        rows,
        [vec![
            Value::Int64(2),
            Value::Text("a".into()),
            Value::Text("A".into())
        ]]
    );
    assert!(resolver.hits.borrow().contains(&1));
    assert!(resolver.hits.borrow().contains(&2));
    // Dynamic domain text has no static field lineage in the enclosing plan.
    let rows = run("SELECT DMin('B','T')='A'", &source, Some(&resolver)).unwrap();
    assert_eq!(rows, [vec![Value::Boolean(false)]]);
    assert!(run("SELECT DMin('U','T')", &source, Some(&resolver)).is_err());
}
