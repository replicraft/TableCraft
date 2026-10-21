//! Collection policy lineage and runtime routing with deliberately divergent policies.
use super::support::*;
use std::cell::Cell;
use tablecraft_query::{
    QueryResult,
    source::{MultivalueList, MultivalueVisitor, ProjectedRow, TextPolicy},
};
use tablecraft_schema::{CollectionMember, Column, ScalarType, SchemaName};

struct Collections {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    values: Vec<Column>,
    members: Vec<CollectionMember>,
    value_policies: Vec<Option<TextCollationId>>,
    member_policies: Vec<Option<TextCollationId>>,
    changed: Cell<u8>,
    scans: Cell<usize>,
    lists: Vec<Vec<Vec<Value>>>,
}
fn text(value: &str) -> Value {
    Value::Text(value.into())
}
impl Collections {
    fn new() -> Self {
        Self {
            columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            unavailable: ["Tags", "Files"]
                .map(|n| SchemaName::new(n.into()).unwrap())
                .to_vec(),
            values: vec![Column::new("Tags".into(), ScalarType::Text, true).unwrap()],
            members: [
                ("Name", ScalarType::Text),
                ("Type", ScalarType::Text),
                ("Size", ScalarType::Int32),
            ]
            .map(|(n, t)| {
                CollectionMember::new("Files".into(), Column::new(n.into(), t, true).unwrap())
                    .unwrap()
            })
            .to_vec(),
            value_policies: vec![Some(TextCollationId::from_host(1))],
            member_policies: vec![
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
                None,
            ],
            changed: Cell::new(0),
            scans: Cell::new(0),
            lists: vec![
                vec![
                    vec![text("a"), text("A"), text("b")],
                    vec![text("a"), text("A"), text("b")],
                    vec![text("a"), text("A"), text("b")],
                    vec![Value::Int32(1); 3],
                ],
                vec![
                    vec![text("b")],
                    vec![text("b")],
                    vec![text("c")],
                    vec![Value::Int32(2)],
                ],
                vec![vec![], vec![], vec![], vec![]],
            ],
        }
    }
    fn visit(
        &self,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for (i, lists) in self.lists.iter().enumerate() {
            let selected = values
                .iter()
                .copied()
                .chain(members.iter().map(|n| n + 1))
                .map(|n| &lists[n] as &dyn MultivalueList)
                .collect::<Vec<_>>();
            visitor(
                ProjectedRow::select(&[Value::Int32(i as i32 + 1)], indices)?,
                &selected,
            )?;
        }
        Ok(())
    }
}
impl Source for Collections {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }
    fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.values)
    }
    fn collection_members(&self, _: &str) -> Result<&[CollectionMember], QueryError> {
        Ok(&self.members)
    }
    fn multivalue_text_collations(
        &self,
        _: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(if self.changed.get() == 1 {
            &[]
        } else {
            &self.value_policies
        })
    }
    fn collection_member_text_collations(
        &self,
        _: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(if self.changed.get() == 2 {
            &[]
        } else {
            &self.member_policies
        })
    }
    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for i in 0..self.lists.len() {
            visitor(&[Value::Int32(i as i32 + 1)])?;
        }
        Ok(())
    }
    fn scan_collection_members(
        &self,
        _: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.visit(indices, values, members, visitor)
    }
}
fn run(
    sql: &str,
    source: &Collections,
    resolver: Option<&Resolver>,
) -> Result<QueryResult, QueryError> {
    let parsed = parse_query(sql, ParseLimits::default()).unwrap();
    let plan = prepare(parsed.query(), source)?;
    plan.execute(
        source,
        &[],
        ExecutionOptions {
            collation: Some(&DEFAULT),
            collation_resolver: resolver.map(|r| r as &dyn CollationResolver),
            like_mode: Some(tablecraft_expressions::predicates::LikeMode::Ansi89),
            ..Default::default()
        },
    )
}

#[test]
fn binding_captures_collection_lineage_without_scanning() {
    let source = Collections::new();
    let parsed = parse_query(
        "SELECT Tags.Value, Files.Name, Files.Type, Files.[Size] FROM T",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(parsed.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        plan.output_text_policies().unwrap(),
        [
            TextPolicy::Column(TextCollationId::from_host(1)),
            TextPolicy::Column(TextCollationId::from_host(1)),
            TextPolicy::Column(TextCollationId::from_host(2)),
            TextPolicy::Default
        ]
    );
}

#[test]
fn expanded_and_hidden_parent_membership_use_element_policy() {
    let source = Collections::new();
    let resolver = Resolver::new();
    for sql in [
        "SELECT Id FROM T WHERE Tags.Value='A'",
        "SELECT Id FROM T WHERE Files.Name='A'",
        "SELECT A.Id FROM T AS A INNER JOIN T AS B ON A.Id=B.Id WHERE A.Tags.Value='A'",
        "SELECT A.Id FROM T AS A WHERE EXISTS(SELECT B.Id FROM T AS B WHERE B.Id=A.Id AND A.Tags.Value='A')",
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap().rows(),
            [vec![Value::Int32(1)]],
            "{sql}"
        );
        assert_eq!(
            run(sql, &source, None).unwrap().rows(),
            [vec![Value::Int32(1)]],
            "{sql}"
        );
    }
    for sql in [
        "SELECT Tags.Value FROM T WHERE Tags.Value='A'",
        "SELECT Files.Name FROM T WHERE Files.Name LIKE 'A'",
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap().rows(),
            [vec![text("a")], vec![text("A")]],
            "{sql}"
        );
        assert_eq!(
            run(sql, &source, None).unwrap().rows(),
            [vec![text("A")]],
            "{sql}"
        );
    }
    assert!(resolver.hits.borrow().iter().all(|id| *id == 1));
}

#[test]
fn retained_item_policy_changes_parent_results_and_correlated_reads() {
    let mut source = Collections::new();
    source.lists[0][0] = vec![text("a")];
    source.lists[0][1] = vec![text("a")];
    source.lists[0][2] = vec![text("a")];
    source.lists[0][3] = vec![Value::Int32(1)];
    let resolver = Resolver::new();
    for sql in [
        "SELECT Id FROM T WHERE Tags.Value='A'",
        "SELECT Id FROM T WHERE Files.Name='A'",
        "SELECT A.Id FROM T AS A INNER JOIN T AS B ON A.Id=B.Id WHERE A.Files.Name='A'",
        "SELECT A.Id FROM T AS A WHERE EXISTS(SELECT B.Id FROM T AS B WHERE B.Id=A.Id AND A.Tags.Value='A')",
    ] {
        assert_eq!(
            run(sql, &source, Some(&resolver)).unwrap().rows(),
            [vec![Value::Int32(1)]],
            "{sql}"
        );
        assert!(run(sql, &source, None).unwrap().rows().is_empty(), "{sql}");
    }
}

#[test]
fn collection_group_distinct_order_aggregate_and_crosstab_route_policies() {
    let source = Collections::new();
    let resolver = Resolver::new();
    assert_eq!(
        run(
            "SELECT DISTINCT Tags.Value AS V FROM T ORDER BY V",
            &source,
            Some(&resolver)
        )
        .unwrap()
        .rows(),
        [vec![Value::Null], vec![text("a")], vec![text("b")]]
    );
    assert_eq!(run("SELECT Files.Name,Count(*) FROM T GROUP BY Files.Name HAVING Count(*)>1 ORDER BY Files.Name",&source,Some(&resolver)).unwrap().rows(),[vec![text("a"),Value::Int64(2)],vec![text("b"),Value::Int64(2)]]);
    assert_eq!(
        run(
            "SELECT Min(Files.Type),Max(Files.Type) FROM T",
            &source,
            Some(&resolver)
        )
        .unwrap()
        .rows(),
        [vec![text("c"), text("A")]]
    );
    assert_eq!(
        run(
            "TRANSFORM Count(*) SELECT 1 AS R FROM T GROUP BY 1 PIVOT Files.Name IN ('A','b')",
            &source,
            Some(&resolver)
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(1), Value::Int64(2), Value::Int64(2)]]
    );
    assert!(resolver.hits.borrow().contains(&1));
    assert!(resolver.hits.borrow().contains(&2));
}

#[test]
fn member_conflicts_unknown_policies_and_global_override_remain_explicit() {
    let mut source = Collections::new();
    let resolver = Resolver::new();
    assert_eq!(
        run(
            "SELECT Files.Name=Files.Type FROM T",
            &source,
            Some(&resolver)
        )
        .err(),
        Some(QueryError::CollationConflict)
    );
    assert!(run("SELECT Files.Name=Files.Type FROM T", &source, None).is_ok());
    source.member_policies[0] = Some(TextCollationId::from_host(999));
    assert!(run("SELECT Files.Name FROM T", &source, Some(&resolver)).is_ok());
    assert_eq!(
        run("SELECT Files.Name='a' FROM T", &source, Some(&resolver)).err(),
        Some(tablecraft_expressions::EvalError::CollationRequired.into())
    );
}

#[test]
fn malformed_collection_policies_fail_before_any_scan() {
    for case in 0..4 {
        let mut source = Collections::new();
        match case {
            0 => source.value_policies.push(None),
            1 => {
                source.member_policies.pop().unwrap();
            }
            2 => source.values[0] = Column::new("Tags".into(), ScalarType::Int32, true).unwrap(),
            _ => source.member_policies[2] = Some(TextCollationId::from_host(1)),
        }
        let parsed = parse_query("SELECT Id FROM T", ParseLimits::default()).unwrap();
        assert_eq!(
            prepare(parsed.query(), &source)
                .err()
                .map(|e| e.to_string()),
            Some(QueryError::Source.to_string())
        );
        assert_eq!(source.scans.get(), 0);
    }
}

#[test]
fn changed_collection_policy_metadata_fails_before_all_scan_paths() {
    for sql in [
        "SELECT Id FROM T",
        "SELECT Tags.Value FROM T",
        "SELECT Id FROM T WHERE Tags.Value='A'",
        "SELECT A.Id FROM T AS A INNER JOIN T AS B ON A.Id=B.Id WHERE A.Tags.Value='A'",
    ] {
        for changed in [1, 2] {
            let source = Collections::new();
            let parsed = parse_query(sql, ParseLimits::default()).unwrap();
            let plan = prepare(parsed.query(), &source).unwrap();
            source.changed.set(changed);
            assert_eq!(
                plan.execute(&source, &[], ExecutionOptions::default())
                    .err(),
                Some(QueryError::SchemaChanged),
                "{sql}"
            );
            assert_eq!(source.scans.get(), 0);
        }
    }
}
