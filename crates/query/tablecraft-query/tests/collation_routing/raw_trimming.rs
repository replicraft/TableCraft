use super::{
    raw_support::{RawResolver, Units},
    support::*,
};

#[test]
fn trimming_keeps_fresh_result_policy_without_activating_input_comparers() {
    let source = Fixture::new();
    let parsed = parse_query(
        "SELECT Trim(A&ChrW(55296))=('A'&ChrW(55296)),Trim(U&ChrW(55296))=Trim(U&ChrW(55296)) FROM T WHERE Id=1",
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
