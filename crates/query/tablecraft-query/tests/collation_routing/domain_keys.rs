use super::support::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::QueryLimits;

#[test]
fn neutral_representatives_do_not_hide_later_conflicts_or_group_output_policies() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let mixed = r#"DLookup(Choose(Id,"'a'",'A','B',"'a'"),'T','Id=1')"#;
    for sql in [
        format!("SELECT DISTINCT {mixed} FROM T"),
        format!("SELECT Count(*) FROM T GROUP BY {mixed}"),
        format!("SELECT Id FROM T ORDER BY {mixed}"),
        "SELECT 'a' AS X UNION SELECT DLookup('A','T') UNION SELECT DLookup('B','T')".into(),
    ] {
        assert_eq!(
            run(&sql, &source, Some(&resolver)).err(),
            Some(QueryError::CollationConflict),
            "{sql}"
        );
    }
    let key = r#"DLookup(Choose(Id,"'a'",'A',"'a'","'a'"),'T','Id=1')"#;
    for output in [
        key.to_string(),
        format!("({key}&'')"),
        format!("(SELECT {key})"),
    ] {
        let sql = format!("SELECT X='A' FROM (SELECT {output} AS X FROM T GROUP BY {key}) AS Q");
        assert_eq!(
            run(&sql, &source, Some(&resolver)).unwrap(),
            [vec![Value::Boolean(true)]],
            "{sql}"
        );
    }
}

fn grouped_domain_source() -> Fixture {
    let mut source = Fixture::new();
    for (row, expression) in source.rows.iter_mut().zip(["'a'", "A", "'a'", "'a'"]) {
        row[3] = Value::Text(expression.into());
    }
    source.policies[3] = Some(TextCollationId::from_host(999));
    source
}

#[test]
fn composed_string_operands_retain_the_group_keys_accumulated_runtime_policy() {
    let source = grouped_domain_source();
    let resolver = Resolver::new();
    let domain = "DLookup(K,'T','Id=1')";
    for operator in ["&", "+"] {
        let key = format!("({domain}{operator}'')");
        for output in [
            format!("{key}&''"),
            format!("{key}+''"),
            format!("CVar({key})&''"),
            format!("CVar({key})+''"),
            format!("''&(SELECT {key})"),
            format!("(SELECT {key})+''"),
        ] {
            let sql =
                format!("SELECT X='A' FROM (SELECT {output} AS X FROM T GROUP BY {key}) AS Q");
            let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
            let scans = source.scans.get();
            let resolutions = resolver.hits.borrow().len();
            let plan = prepare(parsed.query(), &source)
                .unwrap_or_else(|error| panic!("prepare {sql}: {error}"));
            assert_eq!(source.scans.get(), scans);
            assert_eq!(resolver.hits.borrow().len(), resolutions);
            let rows = plan
                .execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&DEFAULT),
                        collation_resolver: Some(&resolver),
                        ..Default::default()
                    },
                )
                .unwrap_or_else(|error| panic!("execute {sql}: {error}"));
            assert_eq!(rows.rows(), [vec![Value::Boolean(true)]], "{sql}");
            assert_eq!(
                run(&sql, &source, None).unwrap(),
                [vec![Value::Boolean(false)]],
                "global override: {sql}"
            );
        }

        for output in [format!("CStr({key})&''"), format!("Left({key},1)+''")] {
            let sql =
                format!("SELECT X='A' FROM (SELECT {output} AS X FROM T GROUP BY {key}) AS Q");
            assert_eq!(
                run(&sql, &source, Some(&resolver)).unwrap(),
                [vec![Value::Boolean(false)]],
                "fresh result: {sql}"
            );
        }
    }
}

#[test]
fn grouped_string_policy_walks_share_cancellation_and_limits() {
    let source = grouped_domain_source();
    let resolver = Resolver::new();
    let parsed = parse_query(
        "SELECT ((DLookup(K,'T','Id=1')&'')+'')='A' FROM T GROUP BY DLookup(K,'T','Id=1')&''",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    let observed = Cell::new(0);
    let count = || {
        observed.set(observed.get() + 1);
        false
    };
    plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&resolver),
            cancelled: Some(&count),
            ..Default::default()
        },
    )
    .unwrap();

    for stop in 1..=observed.get() {
        let calls = Cell::new(0);
        let cancelled = || {
            calls.set(calls.get() + 1);
            calls.get() == stop
        };
        *resolver.budget.borrow_mut() = ResourceBudget::new(0, 0, 10000);
        let result = plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&resolver),
                cancelled: Some(&cancelled),
                ..Default::default()
            },
        );
        assert!(
            matches!(
                result,
                Err(QueryError::Cancelled | QueryError::Evaluation(EvalError::Cancelled))
            ),
            "checkpoint {stop}"
        );
    }

    assert!(matches!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                limits: QueryLimits {
                    bytes: 1,
                    ..Default::default()
                },
                collation: Some(&DEFAULT),
                collation_resolver: Some(&resolver),
                ..Default::default()
            },
        ),
        Err(QueryError::Limit(_))
    ));
}

struct TrimFold;
impl Collation for TrimFold {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left
            .trim_end_matches(' ')
            .bytes()
            .map(|byte| byte.to_ascii_lowercase())
            .cmp(
                right
                    .trim_end_matches(' ')
                    .bytes()
                    .map(|byte| byte.to_ascii_lowercase()),
            ))
    }
}
struct TrimResolver(Cell<usize>);
impl CollationResolver for TrimResolver {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.0.set(self.0.get() + 1);
        if id.host_key() == 1 {
            Ok(&TrimFold)
        } else {
            Err(EvalError::CollationRequired.into())
        }
    }
}

#[test]
fn fixed_pivot_headings_are_rechecked_when_runtime_domain_policy_becomes_known() {
    let source = Fixture::new();
    let resolver = TrimResolver(Cell::new(0));
    let query = parse_query(
        concat!(
            "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 ",
            "PIVOT DLookup('A','T') IN ('a','a ')"
        ),
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(resolver.0.get(), 0);
    let result = plan.execute(
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&resolver),
            ..Default::default()
        },
    );
    assert_eq!(result.err(), Some(QueryError::AmbiguousName));
    assert!(source.scans.get() > 0);
    assert!(resolver.0.get() > 0);
}
