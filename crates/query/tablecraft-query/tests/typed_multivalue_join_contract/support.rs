use std::cell::{Cell, RefCell};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_query::{
    CollectionQueryResult, ExecutionOptions, PreparationOptions, QueryError, QueryValue, Source,
    prepare_with_options,
    source::{
        ExternalSourceRequest, MultivalueList, MultivalueVisitor, ProjectedRow,
        ResolvedExternalSource, SourceKey,
    },
};
use tablecraft_schema::{Column, ScalarType, SchemaName};
use tablecraft_sql::{parse_query, query::Query};

#[derive(Clone)]
pub struct Parent {
    pub scalars: Vec<Value>,
    pub lists: Vec<Vec<Value>>,
}

impl Parent {
    pub fn new(id: i32, key: i32, values: &[Option<i32>], label: &str) -> Self {
        Self {
            scalars: vec![Value::Int32(id), Value::Int32(key)],
            lists: vec![
                values
                    .iter()
                    .map(|value| value.map_or(Value::Null, Value::Int32))
                    .collect(),
                vec![Value::Text(label.into())],
            ],
        }
    }

    pub fn id(&self) -> i32 {
        let Value::Int32(value) = self.scalars[0] else {
            unreachable!()
        };
        value
    }

    pub fn key(&self) -> i32 {
        let Value::Int32(value) = self.scalars[1] else {
            unreachable!()
        };
        value
    }
}

pub struct Table {
    pub name: &'static str,
    columns: Vec<Column>,
    unavailable: Vec<SchemaName>,
    elements: Vec<Column>,
    reordered: Vec<Column>,
    pub rows: Vec<Parent>,
    pub plain: bool,
}

impl Table {
    pub fn new(name: &'static str, rows: Vec<Parent>) -> Self {
        let elements = vec![
            Column::new("A".into(), ScalarType::Int32, true).unwrap(),
            Column::new("B".into(), ScalarType::Text, false).unwrap(),
        ];
        Self {
            name,
            columns: vec![
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("K".into(), ScalarType::Int32, false).unwrap(),
            ],
            // Source metadata orders are deliberately independent.
            unavailable: vec![
                SchemaName::new("B".into()).unwrap(),
                SchemaName::new("A".into()).unwrap(),
            ],
            reordered: elements.iter().rev().cloned().collect(),
            elements,
            rows,
            plain: false,
        }
    }
}

pub struct Data<'a> {
    pub tables: Vec<Table>,
    pub definitions: Vec<(&'static str, Query)>,
    pub external: Vec<(&'static str, u128, &'a dyn Source)>,
    pub active: Cell<bool>,
    pub changed: Cell<bool>,
    pub remap: Cell<bool>,
    pub fail_table: Cell<Option<&'static str>>,
    pub swallow: Cell<bool>,
    pub rejected: Cell<usize>,
    pub scalar_scans: Cell<usize>,
    pub scans: RefCell<Vec<String>>,
    pub external_calls: RefCell<Vec<String>>,
}

impl Data<'_> {
    pub fn new(tables: Vec<Table>) -> Self {
        Self {
            tables,
            definitions: vec![],
            external: vec![],
            active: Cell::new(false),
            changed: Cell::new(false),
            remap: Cell::new(false),
            fail_table: Cell::new(None),
            swallow: Cell::new(false),
            rejected: Cell::new(0),
            scalar_scans: Cell::new(0),
            scans: RefCell::new(vec![]),
            external_calls: RefCell::new(vec![]),
        }
    }

    pub fn fixture() -> Self {
        Self::new(vec![
            Table::new(
                "L",
                vec![
                    Parent::new(1, 7, &[Some(10), Some(10), None], "L0"),
                    Parent::new(1, 8, &[], "L1"),
                    Parent::new(2, 7, &[Some(20)], "L2"),
                ],
            ),
            Table::new(
                "R",
                vec![
                    Parent::new(1, 9, &[Some(100), None], "R0"),
                    Parent::new(1, 7, &[Some(101), Some(101)], "R1"),
                    Parent::new(3, 7, &[], "R2"),
                ],
            ),
            Table::new(
                "S",
                vec![
                    Parent::new(7, 0, &[Some(700)], "S0"),
                    Parent::new(9, 0, &[Some(900)], "S1"),
                ],
            ),
        ])
    }

    pub fn table(&self, name: &str) -> Result<&Table, QueryError> {
        self.tables
            .iter()
            .find(|table| table.name.eq_ignore_ascii_case(name))
            .ok_or(QueryError::UnknownName)
    }

    fn metadata(&self) {
        assert!(
            !self.active.get(),
            "metadata must not reenter an active source cursor"
        );
    }
}

impl Source for Data<'_> {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.metadata();
        Ok(&self.table(name)?.columns)
    }

    fn unavailable_columns(&self, name: &str) -> Result<&[SchemaName], QueryError> {
        self.metadata();
        let table = self.table(name)?;
        Ok(if table.plain { &[] } else { &table.unavailable })
    }

    fn multivalue_columns(&self, name: &str) -> Result<&[Column], QueryError> {
        self.metadata();
        let table = self.table(name)?;
        Ok(if table.plain {
            &[]
        } else if self.changed.get() {
            &table.reordered
        } else {
            &table.elements
        })
    }

    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        self.metadata();
        Ok(self
            .definitions
            .iter()
            .find_map(|(label, definition)| label.eq_ignore_ascii_case(name).then_some(definition)))
    }

    fn external_source(
        &self,
        request: ExternalSourceRequest<'_>,
    ) -> Result<Option<ResolvedExternalSource<'_>>, QueryError> {
        self.metadata();
        self.external_calls
            .borrow_mut()
            .push(request.database().into());
        if request.connection().is_some() || request.connection_bracketed() {
            return Ok(None);
        }
        Ok(self.external.iter().find_map(|(token, key, source)| {
            (*token == request.database()).then(|| {
                ResolvedExternalSource::new(
                    SourceKey::new(*key + u128::from(self.remap.get())),
                    *source,
                )
            })
        }))
    }

    fn scan(
        &self,
        name: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scalar_scans.set(self.scalar_scans.get() + 1);
        self.scans.borrow_mut().push(name.into());
        self.active.set(true);
        let result = (|| {
            for row in &self.table(name)?.rows {
                visitor(&row.scalars)?;
            }
            Ok(())
        })();
        self.active.set(false);
        result
    }

    fn scan_multivalues(
        &self,
        name: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        let table = self.table(name)?;
        assert!(
            !table.plain,
            "a scalar-only source must not need collection callbacks"
        );
        assert!(collections.windows(2).all(|pair| pair[0] < pair[1]));
        self.scans.borrow_mut().push(name.into());
        self.active.set(true);
        let result = (|| {
            for row in &table.rows {
                let lists = collections
                    .iter()
                    .map(|index| &row.lists[*index] as &dyn MultivalueList)
                    .collect::<Vec<_>>();
                if let Err(error) = visitor(ProjectedRow::select(&row.scalars, indices)?, &lists) {
                    self.rejected.set(self.rejected.get() + 1);
                    if !self.swallow.get() {
                        return Err(error);
                    }
                }
            }
            if self
                .fail_table
                .get()
                .is_some_and(|failed| failed.eq_ignore_ascii_case(name))
            {
                return Err(QueryError::Source);
            }
            Ok(())
        })();
        self.active.set(false);
        result
    }
}

pub fn syntax(sql: &str) -> Query {
    parse_query(sql, Default::default()).unwrap().into_query()
}

pub fn execute(
    sql: &str,
    source: &dyn Source,
    declared: bool,
) -> Result<CollectionQueryResult, QueryError> {
    let plan = prepare_with_options(
        &syntax(sql),
        source,
        PreparationOptions {
            declared_arithmetic: declared,
            ..Default::default()
        },
    )?;
    plan.execute_collections(
        source,
        &[],
        ExecutionOptions {
            declared_integral: declared,
            ..Default::default()
        },
    )
}

pub enum Expected {
    Scalar(Value),
    List(Vec<Value>),
}

pub fn scalar(value: i32) -> Expected {
    Expected::Scalar(Value::Int32(value))
}
pub fn list(parent: &Parent, index: usize) -> Expected {
    Expected::List(parent.lists[index].clone())
}

pub fn assert_rows(result: &CollectionQueryResult, expected: &[Vec<Expected>]) {
    assert_eq!(result.rows().len(), expected.len());
    for (actual, expected) in result.rows().iter().zip(expected) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            match (actual, expected) {
                (
                    QueryValue::Scalar(ExpressionValue::Value(actual)),
                    Expected::Scalar(expected),
                ) => assert_eq!(actual, expected),
                (QueryValue::Multivalue(actual), Expected::List(expected)) => {
                    assert_eq!(actual.values(), expected)
                }
                _ => panic!("typed result changed scalar/list shape"),
            }
        }
    }
}
