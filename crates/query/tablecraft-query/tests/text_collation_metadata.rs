use proptest::prelude::*;
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

#[test]
fn bound_output_provenance_tracks_fields_and_explicit_string_preserving_constructs() {
    use tablecraft_query::source::TextPolicy::{Column, Conflict, Default};
    let a = Column(TextCollationId::from_host(1));
    let b = Column(TextCollationId::from_host(2));
    let source = Policies::new(vec![
        None,
        Some(TextCollationId::from_host(1)),
        Some(TextCollationId::from_host(2)),
    ]);
    for (sql, expected) in [
        ("SELECT * FROM T", vec![Default, a, b]),
        (
            "SELECT Label AS AliasValue FROM T ORDER BY AliasValue",
            vec![a],
        ),
        (
            "SELECT q.Label,q.Unused FROM (SELECT Label,Unused FROM T) q",
            vec![a, b],
        ),
        ("SELECT Label FROM Named", vec![a]),
        ("SELECT (SELECT t.Label) AS V FROM T t", vec![a]),
        ("SELECT 'x' & Label,Label+Unused FROM T", vec![a, Conflict]),
        (
            "SELECT IIf(Unused='a',Label,'x'),IIf(TRUE,Label,Unused) FROM T",
            vec![a, Conflict],
        ),
        (
            "SELECT Nz(Label,Unused),Nz(NULL,Label),Nz(Unused) FROM T",
            vec![Conflict, a, b],
        ),
        (
            "SELECT First(Label),Last(Unused),Min(Label),Max(Unused),Count(*) FROM T",
            vec![a, b, a, b, Default],
        ),
        (
            "SELECT Label,Count(*) FROM T GROUP BY Label HAVING Label='a'",
            vec![a, Default],
        ),
        (
            "SELECT Left(Label,1),LCase(Label),CStr(Label),Label='x',Label IS NULL FROM T",
            vec![Default; 5],
        ),
        ("SELECT Label FROM T UNION ALL SELECT 'x'", vec![a]),
        ("SELECT Label FROM T UNION ALL SELECT Label FROM T", vec![a]),
        (
            "SELECT Label FROM T UNION ALL SELECT Unused FROM T",
            vec![Conflict],
        ),
        (
            "SELECT q.V FROM (SELECT Label AS V FROM T UNION ALL SELECT Unused FROM T) q",
            vec![Conflict],
        ),
    ] {
        let parsed = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(parsed.query(), &source).unwrap();
        assert_eq!(
            plan.output_text_policies(),
            Some(expected.as_slice()),
            "{sql}"
        );
        assert_eq!(source.scans.get(), 0);
    }
}
#[test]
fn literal_crosstab_widths_and_named_crosstabs_repeat_the_cell_provenance_without_rows() {
    use tablecraft_query::source::TextPolicy::{Column, Default};
    let mut source = Policies::new(vec![None, Some(TextCollationId::from_host(1)), None]);
    let fixed = "TRANSFORM First(Label) SELECT Id FROM T GROUP BY Id PIVOT Id IN (1,2)";
    let expected = [
        Default,
        Column(TextCollationId::from_host(1)),
        Column(TextCollationId::from_host(1)),
    ];
    let parsed = parse_query(fixed, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(plan.output_text_policies(), Some(expected.as_slice()));
    source.query = parsed.query().clone();
    let named = parse_query("SELECT * FROM Named", ParseLimits::default()).unwrap();
    let plan = prepare(named.query(), &source).unwrap();
    assert_eq!(plan.output_text_policies(), Some(expected.as_slice()));
    let dynamic = parse_query(
        "TRANSFORM First(Label) SELECT Id FROM T GROUP BY Id PIVOT Id",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(dynamic.query(), &source).unwrap();
    assert_eq!(plan.output_text_policies(), None);
    assert_eq!(source.scans.get(), 0);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn generated_string_provenance_matches_an_independent_contributing_key_set(steps in prop::collection::vec(0u8..6,0..12)) {
        use std::collections::BTreeSet;
        use tablecraft_query::source::TextPolicy;
        let source=Policies::new(vec![None,Some(TextCollationId::from_host(1)),Some(TextCollationId::from_host(2))]);
        let mut expression="'seed'".to_string();
        let mut contributors=BTreeSet::new();
        for step in steps {
            expression=match step {
                0=>{contributors.insert(1);format!("({expression}&Label)")},
                1=>{contributors.insert(2);format!("({expression}&Unused)")},
                2=>{contributors.clear();format!("Left({expression},1)")},
                3=>{contributors.insert(2);format!("IIf(Id=1,{expression},Unused)")},
                4=>{contributors.insert(1);format!("Nz({expression},Label)")},
                _=>format!("({expression}&'literal')"),
            };
        }
        let expected=match contributors.len() {
            0=>TextPolicy::Default,
            1=>TextPolicy::Column(TextCollationId::from_host(*contributors.first().unwrap())),
            _=>TextPolicy::Conflict,
        };
        let parsed=parse_query(&format!("SELECT {expression} AS V FROM T"),ParseLimits::default()).unwrap();
        let plan=prepare(parsed.query(),&source).unwrap();
        let expected=[expected];
        prop_assert_eq!(plan.output_text_policies(),Some(expected.as_slice()));
        prop_assert_eq!(source.scans.get(),0);
    }
}

#[test]
fn conflicting_provenance_does_not_change_explicit_global_override_execution() {
    use tablecraft_query::source::TextPolicy;
    let source = Policies::new(vec![
        None,
        Some(TextCollationId::from_host(1)),
        Some(TextCollationId::from_host(2)),
    ]);
    let parsed = parse_query(
        "SELECT IIf(TRUE,Label,Unused) AS V FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(
        plan.output_text_policies(),
        Some([TextPolicy::Conflict].as_slice())
    );
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
        [vec![Value::Text("a".into())]]
    );
}
