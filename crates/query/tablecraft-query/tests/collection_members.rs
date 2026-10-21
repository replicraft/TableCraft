#[path = "collection_members/criteria.rs"]
mod criteria;
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, Source, prepare,
    source::{MultivalueList, MultivalueVisitor, ProjectedRow},
};
use tablecraft_schema::{CollectionMember, Column, ScalarType, SchemaName};
use tablecraft_sql::{ParseLimits, parse_query};

struct Attachments {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    members: Vec<CollectionMember>,
    lists: Vec<Vec<Vec<Value>>>,
    scans: Cell<usize>,
    ignore_errors: bool,
}
impl Attachments {
    fn new() -> Self {
        Self {
            columns: vec![Column::new("Id".into(), ScalarType::Int32, false).unwrap()],
            unavailable: ["Files", "Other"]
                .map(|n| SchemaName::new(n.into()).unwrap())
                .to_vec(),
            // Deliberately interleave two collections in metadata order.
            members: [
                ("Files", "FileName"),
                ("Other", "Label"),
                ("Files", "FileType"),
            ]
            .map(|(c, n)| {
                CollectionMember::new(
                    c.into(),
                    Column::new(n.into(), ScalarType::Text, true).unwrap(),
                )
                .unwrap()
            })
            .to_vec(),
            lists: vec![
                vec![
                    vec![text("a"), text("b")],
                    vec![text("x"), text("y")],
                    vec![text("txt"), text("png")],
                ],
                vec![vec![], vec![Value::Null], vec![]],
                vec![vec![Value::Null], vec![], vec![text("bin")]],
            ],
            scans: Cell::new(0),
            ignore_errors: false,
        }
    }
}
fn text(value: &str) -> Value {
    Value::Text(value.into())
}
impl Source for Attachments {
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }
    fn collection_members(&self, _: &str) -> Result<&[CollectionMember], QueryError> {
        Ok(&self.members)
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
        assert!(values.is_empty());
        self.scans.set(self.scans.get() + 1);
        for (i, lists) in self.lists.iter().enumerate() {
            let selected = members
                .iter()
                .map(|n| &lists[*n] as &dyn MultivalueList)
                .collect::<Vec<_>>();
            let result = visitor(
                ProjectedRow::select(&[Value::Int32(i as i32 + 1)], indices)?,
                &selected,
            );
            if !self.ignore_errors {
                result?;
            }
        }
        Ok(())
    }
}
fn run(sql: &str, source: &Attachments) -> Result<QueryResult, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(query.query(), source)?.execute(source, &[], ExecutionOptions::default())
}
#[test]
fn same_child_members_align_and_independent_collections_multiply() {
    let source = Attachments::new();
    let result = run(
        "SELECT Id,Files.FileName,Files.FileType FROM Items",
        &source,
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), text("a"), text("txt")],
            vec![Value::Int32(1), text("b"), text("png")],
            vec![Value::Int32(2), Value::Null, Value::Null],
            vec![Value::Int32(3), Value::Null, text("bin")],
        ]
    );
    let result = run(
        "SELECT T.Id,T.Files.FileName,T.Other.Label,T.Files.FileType FROM Items AS T",
        &source,
    )
    .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(1), text("a"), text("x"), text("txt")],
            vec![Value::Int32(1), text("a"), text("y"), text("txt")],
            vec![Value::Int32(1), text("b"), text("x"), text("png")],
            vec![Value::Int32(1), text("b"), text("y"), text("png")],
            vec![Value::Int32(2), Value::Null, Value::Null, Value::Null],
            vec![Value::Int32(3), Value::Null, Value::Null, text("bin")],
        ]
    );
    assert_eq!(
        run("SELECT Count(Files.FileType) FROM Items", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int64(3)]]
    );
}
#[test]
fn parent_membership_metadata_binding_and_unsupported_fields_remain_explicit() {
    let source = Attachments::new();
    let query = parse_query("SELECT Files.FileName FROM Items", ParseLimits::default()).unwrap();
    prepare(query.query(), &source).unwrap();
    assert_eq!(source.scans.get(), 0);
    assert_eq!(
        run("SELECT Id FROM Items WHERE Files.FileType IS NULL", &source)
            .unwrap()
            .rows()
            .len(),
        0
    );
    assert_eq!(
        run("SELECT Id FROM Items WHERE Files.FileName IS NULL", &source)
            .unwrap()
            .rows(),
        [vec![Value::Int32(3)]]
    );
    assert_eq!(
        run(
            "SELECT Id,Files.FileType FROM Items WHERE Files.FileName IS NULL",
            &source
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(2), Value::Null],
            vec![Value::Int32(3), text("bin")]
        ]
    );
    for sql in [
        "SELECT Files FROM Items",
        "SELECT * FROM Items",
        "SELECT Files.FileData FROM Items",
    ] {
        assert!(run(sql, &source).is_err(), "{sql}");
    }
}
#[test]
fn invalid_provider_alignment_and_values_latch_errors() {
    let mut source = Attachments::new();
    source.ignore_errors = true;
    source.lists[0][2].pop();
    assert_eq!(
        run("SELECT Files.FileName,Files.FileType FROM Items", &source).err(),
        Some(QueryError::Source)
    );
    assert_eq!(
        run(
            "SELECT Id FROM Items WHERE Files.FileName IS NULL AND Files.FileType IS NULL",
            &source
        )
        .err(),
        Some(QueryError::Source)
    );
    source.lists[0][2].push(Value::Int32(12));
    assert!(matches!(
        run("SELECT Files.FileType FROM Items", &source),
        Err(QueryError::Schema(_))
    ));
}

#[test]
fn legacy_values_and_named_members_use_separate_index_spaces() {
    struct Mixed {
        inner: Attachments,
        values: Vec<Column>,
    }
    impl Source for Mixed {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.inner.columns)
        }
        fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
            Ok(&self.inner.unavailable)
        }
        fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.values)
        }
        fn collection_members(&self, _: &str) -> Result<&[CollectionMember], QueryError> {
            Ok(&self.inner.members)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("collection scan required")
        }
        fn scan_collection_members(
            &self,
            _: &str,
            indices: &[usize],
            values: &[usize],
            members: &[usize],
            visitor: &mut MultivalueVisitor<'_>,
        ) -> Result<(), QueryError> {
            assert_eq!(values, [0]);
            assert_eq!(members, [0, 2]);
            let tags = vec![Value::Int32(7), Value::Int32(8)];
            for (i, lists) in self.inner.lists.iter().enumerate() {
                let mut selected: Vec<&dyn MultivalueList> = vec![&tags];
                selected.extend(members.iter().map(|n| &lists[*n] as &dyn MultivalueList));
                visitor(
                    ProjectedRow::select(&[Value::Int32(i as i32 + 1)], indices)?,
                    &selected,
                )?;
            }
            Ok(())
        }
    }
    let mut source = Mixed {
        inner: Attachments::new(),
        values: vec![Column::new("Tags".into(), ScalarType::Int32, true).unwrap()],
    };
    source
        .inner
        .unavailable
        .push(SchemaName::new("Tags".into()).unwrap());
    let query = parse_query(
        "SELECT Tags.Value,Files.FileName,Files.FileType FROM Items",
        ParseLimits::default(),
    )
    .unwrap();
    let result = prepare(query.query(), &source)
        .unwrap()
        .execute(&source, &[], ExecutionOptions::default())
        .unwrap();
    assert_eq!(
        result.rows(),
        [
            vec![Value::Int32(7), text("a"), text("txt")],
            vec![Value::Int32(7), text("b"), text("png")],
            vec![Value::Int32(8), text("a"), text("txt")],
            vec![Value::Int32(8), text("b"), text("png")],
            vec![Value::Int32(7), Value::Null, Value::Null],
            vec![Value::Int32(8), Value::Null, Value::Null],
            vec![Value::Int32(7), Value::Null, text("bin")],
            vec![Value::Int32(8), Value::Null, text("bin")],
        ]
    );
    for (sql, expected) in [
        (
            "SELECT Tags.Value FROM Items WHERE Files.FileName='a' AND Files.FileType='txt'",
            vec![vec![Value::Int32(7)], vec![Value::Int32(8)]],
        ),
        (
            "SELECT Files.FileName,Files.FileType FROM Items WHERE Tags.Value=7",
            vec![
                vec![text("a"), text("txt")],
                vec![text("b"), text("png")],
                vec![Value::Null, Value::Null],
                vec![Value::Null, text("bin")],
            ],
        ),
    ] {
        let query = parse_query(sql, ParseLimits::default()).unwrap();
        let plan = prepare(query.query(), &source).unwrap();
        assert_eq!(
            plan.execute(
                &source,
                &[],
                ExecutionOptions {
                    collation: Some(&criteria::Ordinal),
                    ..Default::default()
                }
            )
            .unwrap()
            .rows(),
            expected
        );
    }
}

#[test]
fn metadata_changes_duplicates_and_execution_limits_fail_explicitly() {
    let mut source = Attachments::new();
    let query = parse_query(
        "SELECT Files.FileName,Files.FileType FROM Items",
        ParseLimits::default(),
    )
    .unwrap();
    let plan = prepare(query.query(), &source).unwrap();
    for options in [
        ExecutionOptions {
            cancelled: Some(&|| true),
            ..Default::default()
        },
        ExecutionOptions {
            limits: tablecraft_query::QueryLimits {
                rows: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        assert!(plan.execute(&source, &[], options).is_err());
    }
    source.members.pop();
    assert_eq!(
        plan.execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    source.members.push(source.members[0].clone());
    assert!(matches!(
        prepare(query.query(), &source),
        Err(QueryError::Schema(_))
    ));
}

#[path = "collection_members/mixed.rs"]
mod mixed;

#[path = "collection_members/crosstab.rs"]
mod crosstab;

#[path = "collection_members/subqueries.rs"]
mod subqueries;
