use super::support::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_expressions::{EvalError, comparison::Collation};

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
