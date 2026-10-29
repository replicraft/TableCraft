//! Independent query assertions over explicit field identities and real providers.
use std::cell::{Cell, RefCell};
use tablecraft_access::NativeCollationResolver;
use tablecraft_ace::NativeSortOrder;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::{EvalError, predicates::LikeMode};
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare, source::TextCollationId};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

const FRENCH: u64 = 1036;
const GENERAL: u64 = 1033;

struct Words {
    columns: Vec<Column>,
    policies: Vec<Option<TextCollationId>>,
    changed: Vec<Option<TextCollationId>>,
    revision_changed: Cell<bool>,
    scans: Cell<usize>,
    rows: Vec<Vec<Value>>,
    named: Query,
}

impl Words {
    fn new() -> Self {
        let values = [
            Some("cote"),
            Some("coté"),
            Some("côte"),
            Some("côté"),
            Some("COTE"),
            None,
        ];
        Self {
            columns: vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Word".into(), ScalarType::Text, true).unwrap(),
                Column::new("Other".into(), ScalarType::Text, true).unwrap(),
            ],
            policies: vec![
                None,
                Some(TextCollationId::from_host(FRENCH)),
                Some(TextCollationId::from_host(GENERAL)),
            ],
            changed: vec![
                None,
                Some(TextCollationId::from_host(GENERAL)),
                Some(TextCollationId::from_host(GENERAL)),
            ],
            revision_changed: Cell::new(false),
            scans: Cell::new(0),
            rows: values
                .into_iter()
                .enumerate()
                .map(|(index, word)| {
                    let text = word.map_or(Value::Null, |value| Value::Text(value.into()));
                    vec![Value::Int32(index as i32 + 1), text.clone(), text]
                })
                .collect(),
            named: parse_query("SELECT Id,Word,Other FROM T", ParseLimits::default())
                .unwrap()
                .query()
                .clone(),
        }
    }
}

impl Source for Words {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn text_collations(&self, _: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(if self.revision_changed.get() {
            &self.changed
        } else {
            &self.policies
        })
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Named").then_some(&self.named))
    }

    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}

fn budget() -> RefCell<ResourceBudget> {
    RefCell::new(ResourceBudget::new(32 * 1024 * 1024, 0, 10_000_000))
}

fn resolver(pool: &RefCell<ResourceBudget>) -> NativeCollationResolver<'_> {
    NativeCollationResolver::new(
        NativeSortOrder::from_native_bytes([9, 4, 0, 0]),
        pool,
        &|| false,
    )
    .unwrap()
}

fn run(
    source: &Words,
    policy: &NativeCollationResolver<'_>,
    sql: &str,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), source).unwrap();
    plan.execute(
        source,
        &[],
        ExecutionOptions {
            collation: Some(policy.default_collation()),
            collation_resolver: Some(policy),
            like_mode: Some(LikeMode::Ansi89),
            ..Default::default()
        },
    )
    .map(|result| result.rows().to_vec())
}

fn ids(values: &[i32]) -> Vec<Vec<Value>> {
    values
        .iter()
        .map(|&value| vec![Value::Int32(value)])
        .collect()
}

#[test]
fn french_field_order_survives_named_and_derived_sources_beside_general_fields() {
    let source = Words::new();
    let pool = budget();
    let policy = resolver(&pool);
    for sql in [
        "SELECT Id FROM T WHERE Word IS NOT NULL ORDER BY Word,Id",
        "SELECT Id FROM Named WHERE Word IS NOT NULL ORDER BY Word,Id",
        "SELECT Id FROM (SELECT Id,Word FROM Named) AS q WHERE Word IS NOT NULL ORDER BY Word,Id",
    ] {
        assert_eq!(
            run(&source, &policy, sql).unwrap(),
            ids(&[1, 5, 3, 2, 4]),
            "{sql}"
        );
    }
    assert_eq!(
        run(
            &source,
            &policy,
            "SELECT Id FROM T WHERE Other IS NOT NULL ORDER BY Other,Id"
        )
        .unwrap(),
        ids(&[1, 5, 2, 3, 4])
    );
    assert_eq!(
        run(
            &source,
            &policy,
            "SELECT Id FROM Named WHERE Word < 'coté' ORDER BY Id"
        )
        .unwrap(),
        ids(&[1, 3, 5])
    );
}

#[test]
fn grouping_sets_extrema_and_same_policy_joins_reuse_french_keys() {
    let source = Words::new();
    let pool = budget();
    let policy = resolver(&pool);
    assert_eq!(
        run(
            &source,
            &policy,
            "SELECT Count(*) FROM (SELECT DISTINCT Word FROM Named WHERE Word IS NOT NULL) AS q"
        )
        .unwrap(),
        [vec![Value::Int64(4)]]
    );
    assert_eq!(
        run(
            &source,
            &policy,
            "SELECT Word,Count(*) FROM Named WHERE Word IS NOT NULL GROUP BY Word ORDER BY Word"
        )
        .unwrap(),
        [
            vec![Value::Text("cote".into()), Value::Int64(2)],
            vec![Value::Text("côte".into()), Value::Int64(1)],
            vec![Value::Text("coté".into()), Value::Int64(1)],
            vec![Value::Text("côté".into()), Value::Int64(1)],
        ]
    );
    assert_eq!(
        run(&source, &policy, "SELECT Min(Word),Max(Word) FROM T").unwrap(),
        [vec![Value::Text("cote".into()), Value::Text("côté".into())]]
    );
    assert_eq!(
        run(
            &source,
            &policy,
            "SELECT Count(*) FROM T a INNER JOIN Named b ON a.Word=b.Word"
        )
        .unwrap(),
        [vec![Value::Int64(7)]]
    );
}

fn check_long_queries(policy: &NativeCollationResolver<'_>) -> Result<(), QueryError> {
    let mut source = Words::new();
    for length in [256, 1024, 20_000] {
        let first_accent = format!("é{}", "e".repeat(length - 1));
        let last_accent = format!("{}é", "e".repeat(length - 1));
        source.rows = vec![
            vec![Value::Int32(1), Value::Text(last_accent), Value::Null],
            vec![Value::Int32(2), Value::Text(first_accent), Value::Null],
            vec![
                Value::Int32(3),
                Value::Text(format!("{}z", "e".repeat(length))),
                Value::Null,
            ],
            vec![
                Value::Int32(4),
                Value::Text(format!("{}y", "e".repeat(length))),
                Value::Null,
            ],
        ];
        assert_eq!(
            run(&source, policy, "SELECT Id FROM Named ORDER BY Word")?,
            ids(&[2, 1, 4, 3])
        );
        assert_eq!(
            run(
                &source,
                policy,
                "SELECT Count(*) FROM (SELECT DISTINCT Word FROM T) AS q"
            )?,
            [vec![Value::Int64(4)]]
        );
    }
    Ok(())
}

#[test]
fn full_string_comparison_retains_primary_and_reverse_accent_differences_after_255_units() {
    // This cumulative workload exceeds the initial 32 MiB test allowance.
    // An explicit larger comparison pool leaves query defaults unchanged.
    let pool = RefCell::new(ResourceBudget::new(64 * 1024 * 1024, 0, 10_000_000));
    let policy = resolver(&pool);
    check_long_queries(&policy).unwrap();
}

#[test]
fn long_query_comparison_bytes_are_cumulative_and_refuse_without_implicit_reset() {
    let pool = budget();
    let policy = resolver(&pool);
    let error = check_long_queries(&policy).unwrap_err();
    let QueryError::Evaluation(EvalError::Limit(limit)) = error else {
        panic!("Expected the comparison-byte refusal, got {error:?}");
    };
    assert_eq!(limit.resource, Resource::Bytes);
    assert_eq!(limit.remaining, pool.borrow().remaining(Resource::Bytes));
    assert!(limit.remaining < 1024 * 1024);
}

#[test]
fn mixed_and_unknown_policies_refuse_while_bare_projection_stays_inactive() {
    let mut source = Words::new();
    let pool = budget();
    let policy = resolver(&pool);
    assert_eq!(
        run(&source, &policy, "SELECT Id FROM T WHERE Word=Other").err(),
        Some(QueryError::CollationConflict)
    );
    source.policies[1] = Some(TextCollationId::from_host(0x0100040c));
    assert_eq!(
        run(&source, &policy, "SELECT Id FROM T WHERE Word='cote'").err(),
        Some(QueryError::Evaluation(EvalError::CollationRequired))
    );
    assert_eq!(
        run(&source, &policy, "SELECT Word FROM T").unwrap().len(),
        6
    );
}

#[test]
fn french_matching_retains_field_policies_and_separate_binary_controls() {
    let source = Words::new();
    let pool = budget();
    let policy = resolver(&pool);
    for (pattern, ids) in [
        ("*", vec![1, 2, 3, 4, 5]),
        ("?", vec![]),
        ("c*", vec![1, 2, 3, 4, 5]),
    ] {
        let sql = format!("SELECT Id FROM T WHERE Word LIKE '{pattern}'");
        assert_eq!(
            run(&source, &policy, &sql).unwrap(),
            ids.into_iter()
                .map(|id| vec![Value::Int32(id)])
                .collect::<Vec<_>>()
        );
    }
    let french_pool = budget();
    let french = NativeCollationResolver::new(
        NativeSortOrder::from_native_bytes([12, 4, 0, 0]),
        &french_pool,
        &|| false,
    )
    .unwrap();
    assert_eq!(
        run(
            &source,
            &french,
            "SELECT InStr(1,'cote côte','côte',2),InStrRev('côte CÔTE','côte',-1,2)"
        )
        .unwrap(),
        [vec![Value::Int32(6), Value::Int32(6)]]
    );
    assert_eq!(
        run(
            &source,
            &french,
            "SELECT InStr(1,'cote','',2),InStr(1,'cote','ot',0)"
        )
        .unwrap(),
        [vec![Value::Int32(1), Value::Int32(2)]]
    );
    assert_eq!(
        run(&source, &french, "SELECT NULL LIKE '*'").unwrap(),
        [vec![Value::Null]]
    );
}

#[test]
fn metadata_revalidation_refuses_before_scan_and_original_snapshot_remains_reusable() {
    let source = Words::new();
    let pool = budget();
    let policy = resolver(&pool);
    let parsed = parse_query("SELECT Id FROM Named ORDER BY Word", ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    source.revision_changed.set(true);
    let options = ExecutionOptions {
        collation: Some(policy.default_collation()),
        collation_resolver: Some(&policy),
        ..Default::default()
    };
    assert_eq!(
        plan.execute(&source, &[], options).err(),
        Some(QueryError::SchemaChanged)
    );
    assert_eq!(source.scans.get(), 0);
    source.revision_changed.set(false);
    assert_eq!(plan.execute(&source, &[], options).unwrap().rows().len(), 6);
}
