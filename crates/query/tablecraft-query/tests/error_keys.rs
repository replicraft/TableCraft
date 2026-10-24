use proptest::prelude::*;
use std::{cell::Cell, collections::BTreeMap};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, Source},
};
use tablecraft_sql::parse_query;

fn run(sql: &str) -> Result<Vec<Vec<Value>>, QueryError> {
    let source = MemorySource::default();
    let query = parse_query(sql, Default::default()).unwrap();
    prepare(query.query(), &source)?
        .execute(&source, &[], Default::default())
        .map(|result| result.rows().to_vec())
}

const INPUT: &str = "(SELECT CVErr(5) AS E UNION ALL SELECT CVErr(0) UNION ALL SELECT CVErr(5) UNION ALL SELECT CVErr(65535)) AS Q";

#[test]
fn error_groups_distinct_union_and_order_use_standard_codes_without_publishing_numbers() {
    let expected = [
        vec![Value::Int32(0)],
        vec![Value::Int32(5)],
        vec![Value::Int32(65535)],
    ];
    for sql in [
        format!("SELECT CLng(E) FROM {INPUT} GROUP BY E ORDER BY E"),
        format!("SELECT CLng(E) FROM (SELECT DISTINCT E FROM {INPUT}) AS D ORDER BY E"),
        "SELECT CLng(E) FROM (SELECT CVErr(5) AS E UNION SELECT CVErr(0) UNION SELECT CVErr(5) UNION SELECT CVErr(65535)) AS D ORDER BY E".into(),
    ] {
        assert_eq!(run(&sql).unwrap(), expected, "{sql}");
    }
    assert_eq!(
        run(&format!(
            "SELECT CLng(E),Count(*) FROM {INPUT} GROUP BY E ORDER BY E"
        ))
        .unwrap(),
        [
            vec![Value::Int32(0), Value::Int64(1)],
            vec![Value::Int32(5), Value::Int64(2)],
            vec![Value::Int32(65535), Value::Int64(1)]
        ]
    );
    assert_eq!(
        run(&format!("SELECT TOP 2 CLng(E) FROM {INPUT} ORDER BY E")).unwrap(),
        [
            vec![Value::Int32(0)],
            vec![Value::Int32(5)],
            vec![Value::Int32(5)]
        ]
    );
    assert_eq!(
        run(&format!(
            "SELECT TOP 1 CLng(E) FROM {INPUT} ORDER BY E DESC"
        ))
        .unwrap(),
        [vec![Value::Int32(65535)]]
    );
}

#[test]
fn singleton_standard_errors_remain_expression_data_until_explicit_publication() {
    let source = MemorySource::default();
    for sql in [
        "SELECT DISTINCT CVErr(0)",
        "SELECT CVErr(0) AS E ORDER BY E",
        "SELECT CVErr(0) AS E GROUP BY CVErr(0)",
        "SELECT CVErr(0) AS E UNION SELECT CVErr(0)",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(
            plan.execute_expression(&source, &[], Default::default())
                .unwrap()
                .rows(),
            [vec![ExpressionValue::Error(0)]],
            "{sql}"
        );
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(QueryError::Evaluation(EvalError::NonScalarExpression)),
            "{sql}"
        );
    }
}

#[test]
fn mixed_null_and_scalar_keys_are_rejected_in_both_encounter_orders() {
    for other in ["NULL", "0", "'Error 0'", "True"] {
        for (first, second) in [("CVErr(0)", other), (other, "CVErr(0)")] {
            for sql in [
                format!(
                    "SELECT Count(*) FROM (SELECT {first} AS E UNION ALL SELECT {second}) AS Q GROUP BY E"
                ),
                format!(
                    "SELECT DISTINCT E FROM (SELECT {first} AS E UNION ALL SELECT {second}) AS Q"
                ),
                format!("SELECT {first} AS E UNION SELECT {second}"),
                format!(
                    "SELECT IsError(E) FROM (SELECT {first} AS E UNION ALL SELECT {second}) AS Q ORDER BY E"
                ),
            ] {
                assert_eq!(
                    run(&sql).err(),
                    Some(QueryError::Evaluation(EvalError::TypeMismatch)),
                    "{sql}"
                );
            }
        }
    }
}

#[test]
fn late_eager_failures_are_not_hidden_by_duplicates_or_top() {
    for sql in [
        "SELECT DISTINCT CVErr(1/N) FROM (SELECT 1 AS N UNION ALL SELECT 0) AS Q",
        "SELECT TOP 1 CLng(E) FROM (SELECT CVErr(0) AS E UNION ALL SELECT CVErr(1/0)) AS Q ORDER BY E",
        "SELECT Count(*) FROM (SELECT 1 AS N UNION ALL SELECT 0) AS Q GROUP BY CVErr(1/N)",
        "SELECT CVErr(0) UNION SELECT CVErr(1/0)",
    ] {
        assert_eq!(
            run(sql).err(),
            Some(QueryError::Evaluation(EvalError::DivisionByZero)),
            "{sql}"
        );
    }
}

#[test]
fn error_key_work_remains_cumulative_and_cancelled_after_successful_prefixes() {
    let source = MemorySource::default();
    let query = parse_query(
        &format!("SELECT TOP 2 CLng(E),Count(*) FROM {INPUT} GROUP BY E ORDER BY E"),
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let calls = Cell::new(0);
    let count = || {
        calls.set(calls.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    let total = calls.get();
    for stop in [total / 2, total - 2] {
        calls.set(0);
        let cancel = || {
            calls.set(calls.get() + 1);
            calls.get() >= stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&cancel),
                    ..Default::default()
                }
            ),
            Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
        ));
    }
    for limits in [
        QueryLimits {
            bytes: 0,
            ..Default::default()
        },
        QueryLimits {
            steps: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(QueryError::Limit(_))
        ));
    }
}

struct Named {
    definition: tablecraft_sql::query::Query,
}

impl Source for Named {
    fn query(&self, name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
        Ok(name
            .eq_ignore_ascii_case("Saved")
            .then_some(&self.definition))
    }

    fn columns(&self, _: &str) -> Result<&[tablecraft_schema::Column], QueryError> {
        panic!("the named definition has no physical sources")
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("the named definition has no physical sources")
    }
}

#[test]
fn named_crosstab_and_domain_composition_keep_error_keys_as_expression_cells() {
    for (definition, sql, expected) in [
        (
            "SELECT CVErr(5) AS E UNION ALL SELECT CVErr(0) UNION ALL SELECT CVErr(5)",
            "SELECT CLng(E),Count(*) FROM Saved GROUP BY E ORDER BY E",
            vec![
                vec![Value::Int32(0), Value::Int64(1)],
                vec![Value::Int32(5), Value::Int64(2)],
            ],
        ),
        (
            "SELECT CVErr(5) AS E UNION SELECT CVErr(0) UNION SELECT CVErr(5)",
            "SELECT DCount('*','Saved')",
            vec![vec![Value::Int64(2)]],
        ),
        (
            "TRANSFORM Count(*) SELECT CVErr(N) AS G FROM (SELECT 5 AS N UNION ALL SELECT 0 UNION ALL SELECT 5) AS Q GROUP BY CVErr(N) PIVOT 1 IN (1)",
            "SELECT CLng(G),[1] FROM Saved ORDER BY G",
            vec![
                vec![Value::Int32(0), Value::Int64(1)],
                vec![Value::Int32(5), Value::Int64(2)],
            ],
        ),
    ] {
        let source = Named {
            definition: parse_query(definition, Default::default())
                .unwrap()
                .into_query(),
        };
        let query = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(
            plan.execute(&source, &[], Default::default())
                .unwrap()
                .rows(),
            expected,
            "{sql}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x4552524b4559),
        ..Default::default()
    })]

    #[test]
    fn grouped_error_counts_match_independent_integer_code_sets(
        codes in prop::collection::vec(any::<u16>(), 1..12)
    ) {
        let mut counts = BTreeMap::<u16, i64>::new();
        let mut arms = Vec::new();
        for code in codes {
            *counts.entry(code).or_default() += 1;
            arms.push(format!("SELECT CVErr({code}) AS E"));
        }
        let sql = format!("SELECT CLng(E),Count(*) FROM ({}) AS Q GROUP BY E ORDER BY E", arms.join(" UNION ALL "));
        let expected = counts.into_iter().map(|(code, count)| vec![Value::Int32(i32::from(code)), Value::Int64(count)]).collect::<Vec<_>>();
        prop_assert_eq!(run(&sql).unwrap(), expected);
    }
}
