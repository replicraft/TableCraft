use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::Value;
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{ExecutionOptions, QueryError, Source, prepare, source::TextCollationId};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query, query::Query};

struct Policies {
    columns: Vec<Column>,
    policies: Vec<Option<TextCollationId>>,
    changed: Vec<Option<TextCollationId>>,
    switch: Cell<bool>,
    fail: Cell<bool>,
    scans: Cell<usize>,
    query: Query,
}
impl Policies {
    fn new(policies: Vec<Option<TextCollationId>>) -> Self {
        Self {
            columns: vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Label".into(), ScalarType::Text, false).unwrap(),
                Column::new("Unused".into(), ScalarType::Text, true).unwrap(),
            ],
            policies,
            changed: vec![
                None,
                Some(TextCollationId::from_host(2)),
                Some(TextCollationId::from_host(3)),
            ],
            switch: Cell::new(false),
            fail: Cell::new(false),
            scans: Cell::new(0),
            query: parse_query("SELECT Id,Label FROM T", ParseLimits::default())
                .unwrap()
                .query()
                .clone(),
        }
    }
}
impl Source for Policies {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn text_collations(&self, _: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        if self.fail.get() {
            return Err(QueryError::Cancelled);
        }
        Ok(if self.switch.get() {
            &self.changed
        } else {
            &self.policies
        })
    }
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        Ok((name == "Named").then_some(&self.query))
    }
    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        visitor(&[Value::Int32(1), Value::Text("a".into()), Value::Null])
    }
}
struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}
#[test]
fn invalid_metadata_fails_preparation_without_scanning_and_opaque_ids_remain_exact() {
    for metadata in [
        vec![Some(TextCollationId::from_host(1))],
        vec![Some(TextCollationId::from_host(1)), None, None],
    ] {
        let source = Policies::new(metadata);
        let query = parse_query("SELECT Id FROM T", ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(query.query(), &source).err(),
            Some(QueryError::Source)
        );
        assert_eq!(source.scans.get(), 0);
    }
    for key in [0, 1, u64::MAX] {
        assert_eq!(TextCollationId::from_host(key).host_key(), key);
        let source = Policies::new(vec![None, Some(TextCollationId::from_host(key)), None]);
        let query = parse_query("SELECT Label FROM T", ParseLimits::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(source.scans.get(), 0);
        assert_eq!(
            plan.execute(&source, &[], ExecutionOptions::default())
                .unwrap()
                .rows(),
            [vec![Value::Text("a".into())]]
        );
    }
}
#[test]
fn captured_ids_survive_pruning_named_derived_join_union_group_and_subquery_paths() {
    for sql in [
        "SELECT Id FROM T",
        "SELECT Id FROM T WHERE FALSE",
        "SELECT Id FROM T WHERE Id>0",
        "SELECT Id FROM (SELECT Id FROM T) AS q",
        "SELECT a.Id FROM T a INNER JOIN T b ON a.Id=b.Id",
        "SELECT Id FROM T UNION ALL SELECT Id FROM T",
        "SELECT Id FROM T WHERE Id IN (SELECT Id FROM T)",
        "SELECT Id FROM Named",
        "SELECT Count(*) FROM T GROUP BY Label",
    ] {
        let source = Policies::new(vec![None, Some(TextCollationId::from_host(1)), None]);
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        source.switch.set(true);
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&Ordinal),
                    ..Default::default()
                }
            )
            .err(),
            Some(QueryError::SchemaChanged),
            "{sql}"
        );
        assert_eq!(source.scans.get(), 0, "{sql}");
    }
}
#[test]
fn changed_unprojected_metadata_and_provider_errors_are_not_hidden() {
    let mut source = Policies::new(vec![None, Some(TextCollationId::from_host(1)), None]);
    source.changed = vec![
        None,
        Some(TextCollationId::from_host(1)),
        Some(TextCollationId::from_host(3)),
    ];
    let query = parse_query("SELECT Label FROM T", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    source.switch.set(true);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    source.fail.set(true);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::Cancelled)
    );
    assert_eq!(source.scans.get(), 0);
    source.switch.set(false);
    source.fail.set(false);
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .unwrap()
            .rows()
            .len(),
        1
    );
}
#[test]
fn metadata_never_activates_or_changes_the_explicit_query_comparer() {
    let source = Policies::new(vec![None, Some(TextCollationId::from_host(u64::MAX)), None]);
    let query = parse_query("SELECT Label='A' AS R FROM T", ParseLimits::default()).unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    assert!(matches!(
        plan.execute(&source, &[], ExecutionOptions::default()),
        Err(QueryError::Evaluation(EvalError::CollationRequired))
    ));
    assert_eq!(
        plan.execute(
            &source,
            &[],
            ExecutionOptions {
                collation: Some(&Ordinal),
                ..Default::default()
            }
        )
        .unwrap()
        .rows(),
        [vec![Value::Boolean(false)]]
    );
}
