use super::*;
use tablecraft_expressions::functions::conversions::text::NumberFormat;

#[test]
fn aliases_reuse_a_completed_comparison_without_reactivating_its_provider() {
    let source = Records::new();
    let resolver = Resolver(RefCell::new(Vec::new()));
    let syntax = parse_query(
        "SELECT A='a' AS Compared,Compared AS FirstCopy,Compared AS SecondCopy FROM T",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(syntax.query(), &source).unwrap();
    assert!(resolver.0.borrow().is_empty());
    assert_eq!(source.scans.get(), 0);
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Boolean(true); 3],
            vec![Value::Boolean(true); 3],
            vec![Value::Boolean(false); 3],
        ]
    );
    assert_eq!(*resolver.0.borrow(), [1, 1, 1]);
    assert_eq!(source.scans.get(), 1);
}

#[test]
fn aliases_forward_actual_string_policy_and_completed_construction_provenance() {
    let source = Records::new();
    let resolver = Resolver(RefCell::new(Vec::new()));
    for sql in [
        "SELECT A AS Original,Original='A' AS Matches FROM T",
        "SELECT A&IIf(True,2,B) AS Constructed,Constructed='A2' AS Matches FROM T",
        "SELECT Matches FROM (SELECT A&IIf(True,2,B) AS Constructed,Constructed='A2' AS Matches FROM T) Q",
    ] {
        let result = run(
            sql,
            &source,
            ExecutionOptions {
                number_format: Some(NumberFormat::Invariant),
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        )
        .unwrap();
        let matches = result
            .rows()
            .iter()
            .map(|row| row.last().unwrap().clone())
            .collect::<Vec<_>>();
        assert_eq!(
            matches,
            [
                Value::Boolean(true),
                Value::Boolean(true),
                Value::Boolean(false)
            ],
            "{sql}"
        );
    }
    assert!(!resolver.0.borrow().is_empty());

    let result = run(
        "SELECT A&B AS Combined,Combined=Combined AS Matches FROM T",
        &source,
        ExecutionOptions {
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::CollationConflict));
}

#[test]
fn retained_aliases_support_raw_observation_and_later_pair_construction() {
    let source = Records::new();
    for (sql, expected) in [
        (
            "SELECT Units,Code FROM (SELECT ChrW(55296) AS RawValue,LenB(RawValue) AS Units,AscW(RawValue) AS Code) Q",
            vec![Value::Int32(2), Value::Int16(-10240)],
        ),
        (
            "SELECT Completed FROM (SELECT ChrW(55357) AS HighUnit,HighUnit&ChrW(56832) AS Completed) Q",
            vec![Value::Text("😀".into())],
        ),
    ] {
        assert_eq!(
            run(sql, &source, Default::default()).unwrap().rows(),
            [expected],
            "{sql}"
        );
    }
    assert_eq!(
        run(
            "SELECT ChrW(55296) AS RawValue,LenB(RawValue) AS Units",
            &source,
            Default::default()
        )
        .err(),
        Some(QueryError::Evaluation(EvalError::UnrepresentableText)),
    );
}
