use super::support::*;
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::ExpressionValue;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{QueryLimits, execute_with_schema_discovery};

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

struct TrimResolver;

impl CollationResolver for TrimResolver {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        if id.host_key() == 1 {
            Ok(&TrimFold)
        } else {
            Err(EvalError::CollationRequired.into())
        }
    }
}

fn run(
    sql: &str,
    source: &Fixture,
    resolver: Option<&dyn CollationResolver>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    Ok(prepare(parsed.query(), source)?
        .execute(
            source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: resolver,
                ..Default::default()
            },
        )?
        .rows()
        .to_vec())
}

#[test]
fn dynamic_headings_merge_when_later_runtime_policy_equates_earlier_values() {
    let source = Fixture::new();
    let sql = concat!(
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 ",
        "PIVOT DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a '\"),'T','Id=1')"
    );
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);

    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimResolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.columns(), ["G", "a"]);
    assert_eq!(result.rows(), [vec![Value::Int64(1), Value::Int64(4)]]);

    // A final required field policy is sufficient even when no default
    // comparer is installed for the earlier neutral candidates.
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation_resolver: Some(&TrimResolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.columns(), ["G", "a"]);
    assert_eq!(result.rows(), [vec![Value::Int64(1), Value::Int64(4)]]);
}

#[test]
fn coalesced_headings_keep_pre_filter_discovery_and_explicit_override() {
    let source = Fixture::new();
    let pivot = r#"DLookup(Choose(Id,"'a'","'a '",'A',"'a '"),'T','Id=1')"#;
    for (filter, count) in [("", 4), ("WHERE Id<3", 2)] {
        let sql =
            format!("TRANSFORM Count(*) SELECT 1 AS G FROM T {filter} GROUP BY 1 PIVOT {pivot}");
        assert_eq!(
            run(&sql, &source, Some(&TrimResolver)).unwrap(),
            [vec![Value::Int64(1), Value::Int64(count)]]
        );
    }
    let sql = format!("TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT {pivot}");
    assert_eq!(
        run(&sql, &source, None).unwrap(),
        [vec![Value::Int64(1), Value::Int64(2), Value::Int64(2)]]
    );

    // Explicit headings remain a declaration: equivalent names are invalid,
    // rather than silently changing the caller's requested width.
    assert_eq!(
        run(
            &format!("{sql} IN ('a','a ')"),
            &source,
            Some(&TrimResolver)
        )
        .err(),
        Some(QueryError::AmbiguousName)
    );
}

#[test]
fn named_discovery_publishes_the_final_coalesced_schema_without_a_second_scan() {
    let mut source = Fixture::new();
    source.named = parse_query(
        concat!(
            "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 ",
            "PIVOT DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a '\"),'T','Id=1')"
        ),
        ParseLimits::default(),
    )
    .unwrap()
    .into_query();
    let outer = parse_query("SELECT * FROM Named", ParseLimits::default()).unwrap();
    assert!(prepare(outer.query(), &source).is_err());
    assert_eq!(source.scans.get(), 0);

    let result = execute_with_schema_discovery(
        outer.query(),
        &source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: Some(&TrimResolver),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(result.columns(), ["G", "a"]);
    assert_eq!(
        result.rows(),
        [vec![
            ExpressionValue::Value(Value::Int64(1)),
            ExpressionValue::Value(Value::Int64(4)),
        ]]
    );
    // One parent scan and exactly one explicit domain lookup per parent row.
    assert_eq!(source.scans.get(), 5);
}

#[test]
fn final_heading_coalescing_shares_cumulative_limits_and_cancellation() {
    let source = Fixture::new();
    let parsed = parse_query(
        concat!(
            "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 ",
            "PIVOT DLookup(Choose(Id,\"'a'\",\"'a '\",'A',\"'a '\"),'T','Id=1')"
        ),
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
            collation_resolver: Some(&TrimResolver),
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
        assert!(
            matches!(
                plan.execute(
                    &source,
                    &[],
                    ExecutionOptions {
                        collation: Some(&DEFAULT),
                        collation_resolver: Some(&TrimResolver),
                        cancelled: Some(&cancelled),
                        ..Default::default()
                    },
                ),
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
                collation_resolver: Some(&TrimResolver),
                ..Default::default()
            },
        ),
        Err(QueryError::Limit(_))
    ));
}

#[test]
fn final_policy_merges_case_equivalent_names_and_preserves_first_spelling() {
    let source = Fixture::new();
    for (choices, name) in [
        (r#""'a'","'A'",'A',"'a '""#, "a"),
        (r#""'A'","'a '","'a'",'A'"#, "A"),
        (r#""'a '","'A'",'A',"'a'""#, "a "),
    ] {
        let sql = format!(
            "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT DLookup(Choose(Id,{choices}),'T','Id=1')"
        );
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&DEFAULT),
                    collation_resolver: Some(&TrimResolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.columns(), ["G", name]);
        assert_eq!(result.rows(), [vec![Value::Int64(1), Value::Int64(4)]]);
    }
}

#[test]
fn stricter_late_policy_recovers_candidates_merged_by_the_default() {
    let source = Fixture::new();
    let resolver = Resolver::new();
    let pivot = r#"DLookup(Choose(Id,"'a'","'a '",'K',"'a'"),'T','Id=1')"#;
    for (filter, expected) in [
        ("", vec![Value::Int64(1), Value::Int64(3), Value::Int64(1)]),
        (
            "WHERE Id<3",
            vec![Value::Int64(1), Value::Int64(1), Value::Int64(1)],
        ),
    ] {
        let sql =
            format!("TRANSFORM Count(*) SELECT 1 AS G FROM T {filter} GROUP BY 1 PIVOT {pivot}");
        let parsed = parse_query(&sql, ParseLimits::default()).unwrap();
        let result = prepare(parsed.query(), &source)
            .unwrap()
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&TrimFold),
                    collation_resolver: Some(&resolver),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(result.columns(), ["G", "a", "a "]);
        assert_eq!(result.rows(), [expected]);
    }
}

struct SingleRowDomain<'a>(&'a Fixture);

impl Source for SingleRowDomain<'_> {
    fn columns(&self, name: &str) -> Result<&[tablecraft_schema::Column], QueryError> {
        self.0.columns(name)
    }

    fn text_collations(&self, name: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        self.0.text_collations(name)
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if name == "One" {
            self.0.scans.set(self.0.scans.get() + 1);
            visitor(self.0.rows.first().ok_or(QueryError::Source)?)
        } else {
            self.0.scan(name, visitor)
        }
    }
}

#[test]
fn candidate_retention_does_not_apply_the_final_schema_width_limit_early() {
    let mut fixture = Fixture::new();
    fixture.rows.clear();
    for mask in 0..256 {
        let spelling: String = "abcdefgh"
            .bytes()
            .enumerate()
            .map(|(position, byte)| {
                if mask & (1 << position) == 0 {
                    char::from(byte)
                } else {
                    char::from(byte.to_ascii_uppercase())
                }
            })
            .collect();
        fixture.rows.push(vec![
            Value::Int32(mask + 1),
            Value::Text("abcdefgh".into()),
            Value::Text("unused".into()),
            Value::Text(format!("'{spelling}'")),
            Value::Text("unused".into()),
        ]);
    }
    let mut last = fixture.rows[0].clone();
    last[0] = Value::Int32(257);
    last[3] = Value::Text("A".into());
    fixture.rows.push(last);
    let source = SingleRowDomain(&fixture);
    let parsed = parse_query(
        "TRANSFORM Count(*) SELECT 1 AS G FROM T GROUP BY 1 PIVOT DLookup(K,'One')",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(fixture.scans.get(), 0);
    let result = plan
        .execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimResolver),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.columns(), ["G", "abcdefgh"]);
    assert_eq!(result.rows(), [vec![Value::Int64(1), Value::Int64(257)]]);
    assert_eq!(fixture.scans.get(), 258);

    // When the final policy leaves more than 255 distinct pivot values,
    // the existing final 256-column schema ceiling still applies.
    fixture.rows.last_mut().unwrap()[3] = Value::Text("'other'".into());
    let source = SingleRowDomain(&fixture);
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&DEFAULT),
                collation_resolver: Some(&TrimResolver),
                ..Default::default()
            },
        )
        .err(),
        Some(QueryError::StructureLimit)
    );
}
