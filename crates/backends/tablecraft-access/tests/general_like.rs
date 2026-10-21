use tablecraft_access::GeneralCollation;
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile},
    predicates::{LikeMode, like},
};
use tablecraft_query::{ExecutionOptions, prepare, source::MemorySource};
use tablecraft_sql::{ParseLimits, parse_query};

fn policy(version: u8, bytes: u64) -> GeneralCollation<'static> {
    GeneralCollation::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, version]),
        ResourceBudget::new(bytes, 0, 1_000_000),
        &|| false,
    )
    .unwrap()
}
#[test]
fn class_and_literal_callbacks_retain_raw_units_and_pattern_spaces() {
    for version in [0, 1] {
        let policy = policy(version, 16 * 1024 * 1024);
        assert_eq!(policy.like_profile(), LikeProfile::Utf16Weights);
        assert!(policy.is_like_unit(&[115, 115]).unwrap());
        assert_eq!(
            policy.compare_like_units(&[115, 115], &[0xdf]).unwrap(),
            std::cmp::Ordering::Equal
        );
        assert_ne!(
            policy.compare_like_units(&[97, 32], &[97]).unwrap(),
            std::cmp::Ordering::Equal
        );
        assert!(policy.compare_like_units(&[0xd800], &[0xd800]).is_ok());
    }
}
#[test]
fn nested_grouped_and_correlated_queries_share_the_general_like_profile() {
    for version in [0, 1] {
        let policy = policy(version, 16 * 1024 * 1024);
        let source = MemorySource::default();
        let sql = "SELECT q.V LIKE '[ß]' AS MatchValue,Count(*) AS N FROM (SELECT 'ss' AS V UNION ALL SELECT 'ß' UNION ALL SELECT 'road') AS q GROUP BY q.V LIKE '[ß]' HAVING q.V LIKE '[ß]'";
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&policy),
                    like_mode: Some(LikeMode::Ansi89),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(
            result.rows(),
            &[vec![Value::Boolean(true), Value::Int64(2)]]
        );
        let parsed=parse_query("SELECT q.ID FROM (SELECT 1 AS ID,'ss' AS V UNION ALL SELECT 2,'road') AS q WHERE EXISTS(SELECT 1 WHERE q.V LIKE '[ß]')",ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&policy),
                    like_mode: Some(LikeMode::Ansi89),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.rows(), &[vec![Value::Int64(1)]]);
    }
}
#[test]
fn general_like_callbacks_consume_one_cumulative_provider_budget() {
    let policy = policy(1, 1000);
    let mut calls = 0;
    loop {
        calls += 1;
        let result = like(
            &Value::Text("ss".into()),
            &Value::Text("ß".into()),
            Some(LikeMode::Ansi89),
            Some(&policy),
            &mut ResourceBudget::new(100000, 0, 100000),
            None,
        );
        match result {
            Ok(_) => assert!(calls < 20),
            Err(EvalError::Limit(_)) => break,
            other => panic!("unexpected result {other:?}"),
        }
    }
    assert!(matches!(
        policy.is_like_unit(&[115, 115]),
        Err(EvalError::Limit(_))
    ));
}
