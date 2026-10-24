use super::{
    raw_support::{RawResolver, Units},
    support::*,
};

#[test]
fn string_result_policy_is_fresh_and_does_not_activate_unselected_source_policies() {
    let source = Fixture::new();
    let parsed = parse_query(
        "SELECT (String(1,A&ChrW(55296))&ChrW(55296))=('A'&ChrW(55296)),(String(1,U&ChrW(55296))&ChrW(55296))=(String(1,U&ChrW(55296))&ChrW(55296)) FROM T WHERE Id=1",
        Default::default(),
    ).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Units(false)),
                collation_resolver: Some(&RawResolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        &[vec![Value::Boolean(false), Value::Boolean(true)]]
    );
}
