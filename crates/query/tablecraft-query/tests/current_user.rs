use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::session::AccessSessionContext,
};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

fn source() -> MemorySource {
    MemorySource::new(vec![
        MemoryTable::new(
            "Items".into(),
            vec![Column::new("ID".into(), ScalarType::Int32, false).unwrap()],
            vec![vec![Value::Int32(1)], vec![Value::Int32(2)]],
        )
        .unwrap(),
    ])
    .unwrap()
}

#[test]
fn session_names_are_execution_scoped_through_grouping_domain_and_dynamic_queries() {
    let source = source();
    for sql in [
        "SELECT CurrentUser(),(SELECT CurrentUser()),Eval('CurrentUser()'),TypeName(CurrentUser())",
        "SELECT CurrentUser(),Count(*) FROM Items GROUP BY CurrentUser()",
        "SELECT CurrentUser(),Count(*) FROM Items GROUP BY Eval('CurrentUser()')",
        "SELECT DFirst('CurrentUser()','Items'),DLast('CurrentUser()','Items'),DLookup('CurrentUser()','Items')",
        "SELECT Q.Account FROM (SELECT CurrentUser() AS Account) AS Q",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        for name in ["Reader", "é中🦀", " e\u{301} "] {
            let session = AccessSessionContext::new(name).unwrap();
            let result = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        access_session: Some(session),
                        collation: Some(&Ordinal),
                        ..Default::default()
                    },
                )
                .unwrap();
            assert_eq!(result.rows()[0][0], Value::Text(name.into()), "{sql}");
        }
        assert_eq!(
            plan.execute(&source, &[], Default::default()).err(),
            Some(EvalError::AccessSessionRequired.into()),
            "{sql}"
        );
    }
}

#[test]
fn invalid_calls_are_rejected_without_evaluating_their_operands() {
    let source = source();
    for sql in [
        "SELECT CurrentUser(1/0)",
        "SELECT CurrentUser(*)",
        "SELECT CurrentUser(,)",
        "SELECT VBA.CurrentUser()",
    ] {
        let query = parse_query(sql, Default::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
    let query = parse_query("SELECT CurrentUser(),1/0", Default::default()).unwrap();
    assert_eq!(
        prepare(query.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    access_session: Some(AccessSessionContext::unsecured()),
                    ..Default::default()
                }
            )
            .err(),
        Some(EvalError::DivisionByZero.into())
    );
}

#[test]
fn materialization_and_nested_queries_share_quotas_and_cancellation() {
    let source = source();
    let query = parse_query(
        "SELECT CurrentUser(),Eval('CurrentUser()'),DFirst('CurrentUser()','Items')",
        Default::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    let session = AccessSessionContext::unsecured();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            access_session: Some(session),
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();
    for stop in 1..=observed.get() {
        let checks = Cell::new(0);
        let cancelled = || {
            checks.set(checks.get() + 1);
            checks.get() >= stop
        };
        assert!(matches!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    access_session: Some(session),
                    cancelled: Some(&cancelled),
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
            steps: 1,
            ..Default::default()
        },
    ] {
        assert!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    access_session: Some(session),
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}
