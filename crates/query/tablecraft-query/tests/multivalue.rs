use proptest::prelude::*;
use std::cell::{Cell, RefCell};
use tablecraft_core::values::Value;
use tablecraft_db::{MultivalueList, ProjectedRow};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryResult, Source, prepare,
    source::{MultivalueVisitor, ScanConstraint},
};
use tablecraft_schema::{Column, ScalarType, SchemaName};
use tablecraft_sql::{ParseLimits, parse_query};

const CHANGED_TEXT_POLICIES: [Option<tablecraft_query::source::TextCollationId>; 2] = [
    Some(tablecraft_query::source::TextCollationId::from_host(7)),
    None,
];

type Request = (Vec<usize>, Vec<usize>);
struct Collections {
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    values: Vec<Column>,
    rows: Vec<Vec<Value>>,
    lists: Vec<Vec<Vec<Value>>>,
    requests: RefCell<Vec<Request>>,
    ordinary: Cell<usize>,
    changed: Cell<bool>,
    policy_changed: Cell<bool>,
    ignore_errors: bool,
}
impl Collections {
    fn new() -> Self {
        Self {
            columns: ["Unused", "Id"]
                .map(|name| Column::new(name.into(), ScalarType::Int32, false).unwrap())
                .to_vec(),
            unavailable: ["Tags", "Other"]
                .map(|name| SchemaName::new(name.into()).unwrap())
                .to_vec(),
            values: ["Tags", "Other"]
                .map(|name| Column::new(name.into(), ScalarType::Int32, true).unwrap())
                .to_vec(),
            rows: vec![
                vec![Value::Int32(99), Value::Int32(1)],
                vec![Value::Int32(99), Value::Int32(2)],
                vec![Value::Int32(99), Value::Int32(3)],
            ],
            lists: vec![
                vec![
                    vec![Value::Int32(7), Value::Int32(8), Value::Int32(8)],
                    vec![Value::Int32(4), Value::Int32(5)],
                ],
                vec![vec![], vec![Value::Int32(6)]],
                vec![vec![Value::Null], vec![]],
            ],
            requests: RefCell::new(vec![]),
            ordinary: Cell::new(0),
            changed: Cell::new(false),
            policy_changed: Cell::new(false),
            ignore_errors: false,
        }
    }
}
impl Source for Collections {
    fn text_collations(
        &self,
        _: &str,
    ) -> Result<&[Option<tablecraft_query::source::TextCollationId>], QueryError> {
        Ok(if self.policy_changed.get() {
            &CHANGED_TEXT_POLICIES
        } else {
            &[]
        })
    }
    fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }
    fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&self.unavailable)
    }
    fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
        Ok(if self.changed.get() {
            &[]
        } else {
            &self.values
        })
    }
    fn scan(
        &self,
        _: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.ordinary.set(self.ordinary.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
    fn scan_projected(
        &self,
        _: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(constraint.is_none());
        self.ordinary.set(self.ordinary.get() + 1);
        for row in &self.rows {
            visitor(ProjectedRow::select(row, indices)?)?;
        }
        Ok(())
    }
    fn scan_multivalues(
        &self,
        _: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.requests
            .borrow_mut()
            .push((indices.to_vec(), collections.to_vec()));
        for (row, lists) in self.rows.iter().zip(&self.lists) {
            let selected = collections
                .iter()
                .map(|index| &lists[*index] as &dyn MultivalueList)
                .collect::<Vec<_>>();
            let result = visitor(ProjectedRow::select(row, indices)?, &selected);
            if !self.ignore_errors {
                result?;
            }
        }
        Ok(())
    }
}
fn run(sql: &str, source: &dyn Source) -> Result<QueryResult, QueryError> {
    let query = parse_query(sql, ParseLimits::default()).unwrap();
    prepare(query.query(), source)?.execute(source, &[], ExecutionOptions::default())
}
#[test]
fn qualified_and_unqualified_members_expand_once_and_keep_duplicates_nulls_and_empty_parents() {
    let source = Collections::new();
    for field in ["Tags.Value", "T.Tags.Value", "[T].[Tags].[Value]"] {
        let sql = format!("SELECT Id,{field} AS V,{field} AS Again FROM Items AS T ORDER BY Id,V");
        let result = run(&sql, &source).unwrap();
        assert_eq!(
            result.rows(),
            [
                vec![Value::Int32(1), Value::Int32(7), Value::Int32(7)],
                vec![Value::Int32(1), Value::Int32(8), Value::Int32(8)],
                vec![Value::Int32(1), Value::Int32(8), Value::Int32(8)],
                vec![Value::Int32(2), Value::Null, Value::Null],
                vec![Value::Int32(3), Value::Null, Value::Null],
            ]
        );
    }
    assert!(
        source
            .requests
            .borrow()
            .iter()
            .all(|request| *request == (vec![1], vec![0]))
    );
    assert_eq!(source.ordinary.get(), 0);
}
#[test]
fn unused_collections_do_not_multiply_scalar_rows_and_plain_collections_stay_unavailable() {
    let source = Collections::new();
    assert_eq!(
        run("SELECT Id FROM Items", &source).unwrap().rows().len(),
        3
    );
    assert_eq!(
        run("SELECT Count(*) FROM Items", &source).unwrap().rows(),
        [vec![Value::Int64(3)]]
    );
    assert!(source.requests.borrow().is_empty());
    for sql in [
        "SELECT * FROM Items",
        "SELECT T.* FROM Items AS T",
        "SELECT Tags FROM Items",
        "SELECT T.Tags FROM Items AS T",
    ] {
        assert_eq!(
            run(sql, &source).err(),
            Some(QueryError::Unsupported),
            "{sql}"
        );
    }
}
#[test]
fn member_filters_grouping_distinct_aggregates_and_crosstabs_use_flattened_rows() {
    let source = Collections::new();
    assert_eq!(
        run(
            "SELECT Id,Tags.Value FROM Items WHERE Tags.Value=8 ORDER BY Id",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        2
    );
    assert_eq!(run("SELECT Tags.Value AS V,Count(*),Count(Tags.Value) FROM Items GROUP BY Tags.Value ORDER BY V",&source).unwrap().rows(),[
        vec![Value::Null,Value::Int64(2),Value::Int64(0)],vec![Value::Int32(7),Value::Int64(1),Value::Int64(1)],vec![Value::Int32(8),Value::Int64(2),Value::Int64(2)]
    ]);
    assert_eq!(
        run(
            "SELECT DISTINCT Tags.Value AS V FROM Items ORDER BY V",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        3
    );
    assert_eq!(
        run(
            "SELECT Id,Count(Tags.Value) FROM Items GROUP BY Id ORDER BY Id",
            &source
        )
        .unwrap()
        .rows(),
        [
            vec![Value::Int32(1), Value::Int64(3)],
            vec![Value::Int32(2), Value::Int64(0)],
            vec![Value::Int32(3), Value::Int64(0)]
        ]
    );
    assert_eq!(
        run(
            "TRANSFORM Count(*) SELECT 1 AS R FROM Items GROUP BY 1 PIVOT Tags.Value IN (7,8)",
            &source
        )
        .unwrap()
        .rows(),
        [vec![Value::Int64(1), Value::Int64(1), Value::Int64(2)]]
    );
}
#[test]
fn multiple_collections_cross_products_and_joins_keep_independent_scopes() {
    let source = Collections::new();
    assert_eq!(
        run(
            "SELECT Id,Tags.Value AS T,Other.Value AS O FROM Items ORDER BY Id,T,O",
            &source
        )
        .unwrap()
        .rows()
        .len(),
        8
    );
    assert_eq!(run("SELECT A.Id,A.Tags.Value FROM Items AS A INNER JOIN Items AS B ON A.Id=B.Id WHERE A.Id=1",&source).unwrap().rows().len(),3);
    assert_eq!(run("SELECT A.Id,A.Tags.Value FROM Items AS A LEFT JOIN Items AS B ON A.Id=B.Id+10 ORDER BY A.Id",&source).unwrap().rows().len(),5);
    assert_eq!(
        run("SELECT Tags.Value FROM Items AS A,Items AS B", &source).err(),
        Some(QueryError::AmbiguousName)
    );
    assert_eq!(
        run("SELECT (SELECT A.Tags.Value) FROM Items AS A", &source)
            .unwrap()
            .rows()
            .len(),
        5
    );
}
#[test]
fn collection_schema_and_element_errors_never_become_successful_partial_results() {
    let mut source = Collections::new();
    let query = parse_query("SELECT Tags.Value FROM Items", ParseLimits::default()).unwrap();
    let prepared = prepare(query.query(), &source).unwrap();
    source.changed.set(true);
    assert_eq!(
        prepared
            .execute(&source, &[], ExecutionOptions::default())
            .err(),
        Some(QueryError::SchemaChanged)
    );
    source.changed.set(false);
    source.lists[1][0].push(Value::Text("wrong".into()));
    source.ignore_errors = true;
    assert!(matches!(
        prepared.execute(&source, &[], ExecutionOptions::default()),
        Err(QueryError::Schema(_))
    ));
    let source = Collections::new();
    let prepared = prepare(query.query(), &source).unwrap();
    assert_eq!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    cancelled: Some(&|| true),
                    ..Default::default()
                }
            )
            .err(),
        Some(QueryError::Cancelled)
    );
    assert!(
        prepared
            .execute(
                &source,
                &[],
                ExecutionOptions {
                    limits: tablecraft_query::QueryLimits {
                        rows: 2,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )
            .is_err()
    );
}
proptest! {
    #![proptest_config(proptest::test_runner::Config { failure_persistence: None, ..Default::default() })]
    #[test]
    fn generated_collections_match_independent_parent_value_expansion(values in prop::collection::vec(prop::collection::vec(prop::option::of(-20_i32..20),0..8),0..10)){
        let mut source=Collections::new();source.rows=values.iter().enumerate().map(|(i,_)|vec![Value::Int32(99),Value::Int32(i as i32)]).collect();
        source.lists=values.iter().map(|list|vec![list.iter().map(|value|value.map_or(Value::Null,Value::Int32)).collect(),vec![]]).collect();
        let expected=values.iter().enumerate().flat_map(|(i,list)|{let values=if list.is_empty(){vec![None]}else{list.clone()};values.into_iter().map(move|value|vec![Value::Int32(i as i32),value.map_or(Value::Null,Value::Int32)])}).collect::<Vec<_>>();
        let actual=run("SELECT Id,Tags.Value FROM Items",&source).unwrap();prop_assert_eq!(actual.rows(),expected);
    }
}

#[test]
fn metadata_binding_does_not_scan_and_member_names_do_not_override_ordinary_qualified_fields() {
    let mut source = Collections::new();
    let query = parse_query("SELECT Tags.Value FROM Items", ParseLimits::default()).unwrap();
    assert!(prepare(query.query(), &source).is_ok());
    assert_eq!(source.ordinary.get(), 0);
    assert!(source.requests.borrow().is_empty());
    source.values.push(source.values[0].clone());
    assert!(matches!(
        prepare(query.query(), &source),
        Err(QueryError::Schema(_))
    ));
    source.values.pop();
    source.values[0] = Column::new("Undeclared".into(), ScalarType::Int32, true).unwrap();
    assert_eq!(
        prepare(query.query(), &source).err(),
        Some(QueryError::Source)
    );
    source.values[0] = Column::new("Tags".into(), ScalarType::Int32, true).unwrap();
    source
        .columns
        .push(Column::new("Value".into(), ScalarType::Int32, false).unwrap());
    for row in &mut source.rows {
        row.push(Value::Int32(999));
    }
    assert_eq!(
        run("SELECT Tags.Value FROM Items AS Tags", &source).err(),
        Some(QueryError::AmbiguousName)
    );
    assert_eq!(
        run("SELECT Tags.Tags.Value FROM Items AS Tags", &source)
            .unwrap()
            .rows()
            .len(),
        5
    );
}

#[test]
fn invalid_list_contracts_fail_even_if_a_provider_ignores_the_visitor() {
    struct Missing;
    impl MultivalueList for Missing {
        fn len(&self) -> usize {
            1
        }
        fn get(&self, _: usize) -> Option<&Value> {
            None
        }
    }
    struct Broken {
        source: Collections,
        wrong_width: bool,
    }
    impl Source for Broken {
        fn columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.source.columns)
        }
        fn unavailable_columns(&self, _: &str) -> Result<&[SchemaName], QueryError> {
            Ok(&self.source.unavailable)
        }
        fn multivalue_columns(&self, _: &str) -> Result<&[Column], QueryError> {
            Ok(&self.source.values)
        }
        fn scan(
            &self,
            _: &str,
            _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
        ) -> Result<(), QueryError> {
            panic!("expanded scan required")
        }
        fn scan_multivalues(
            &self,
            _: &str,
            indices: &[usize],
            _: &[usize],
            visitor: &mut MultivalueVisitor<'_>,
        ) -> Result<(), QueryError> {
            for row in &self.source.rows {
                let lists: Vec<&dyn MultivalueList> = if self.wrong_width {
                    vec![]
                } else {
                    vec![&Missing]
                };
                let _ = visitor(ProjectedRow::select(row, indices)?, &lists);
            }
            Ok(())
        }
    }
    for wrong_width in [false, true] {
        assert_eq!(
            run(
                "SELECT Tags.Value FROM Items",
                &Broken {
                    source: Collections::new(),
                    wrong_width
                }
            )
            .err(),
            Some(QueryError::Source)
        );
    }
}

#[test]
fn collection_only_sources_keep_parent_cardinality_without_scalar_placeholders() {
    let mut source = Collections::new();
    source.columns.clear();
    for row in &mut source.rows {
        row.clear();
    }
    assert_eq!(
        run("SELECT Tags.Value FROM Items", &source)
            .unwrap()
            .rows()
            .len(),
        5
    );
    assert_eq!(
        run("SELECT Count(*) FROM Items", &source).unwrap().rows(),
        [vec![Value::Int64(3)]]
    );
    source.values[0] = Column::new("Tags".into(), ScalarType::Int32, false).unwrap();
    assert!(matches!(
        run("SELECT Tags.Value FROM Items", &source),
        Err(QueryError::Schema(_))
    ));
}

#[path = "multivalue/joined_parent.rs"]
mod joined_parent;
#[path = "multivalue/parent.rs"]
mod parent;

#[path = "multivalue/mixed.rs"]
mod mixed;

#[path = "multivalue/crosstab.rs"]
mod crosstab;

#[path = "multivalue/subqueries.rs"]
mod subqueries;

#[path = "multivalue/joins.rs"]
mod joins;

#[path = "multivalue/products.rs"]
mod products;

#[path = "multivalue/ordering.rs"]
mod ordering;
