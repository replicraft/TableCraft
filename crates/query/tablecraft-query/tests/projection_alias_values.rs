//! Independent value, declaration and policy checks for prior projection aliases.
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryResult, Source, prepare,
    source::{CollationResolver, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "projection_alias_values/boundaries.rs"]
mod boundaries;
#[path = "projection_alias_values/policies.rs"]
mod policies;
#[path = "projection_alias_values/types.rs"]
mod types;

struct Records {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    scans: Cell<usize>,
}

impl Records {
    fn new() -> Self {
        Self {
            columns: [
                ("N", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
            ]
            .into_iter()
            .map(|(name, kind)| Column::new(name.into(), kind, false).unwrap())
            .collect(),
            rows: [(1, "a"), (2, "A"), (3, "b")]
                .into_iter()
                .map(|(number, text)| {
                    vec![
                        Value::Int32(number),
                        Value::Text(text.into()),
                        Value::Text(text.into()),
                    ]
                })
                .collect(),
            policies: vec![
                None,
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
            ],
            scans: Cell::new(0),
        }
    }
}

impl Source for Records {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.policies)
    }

    fn scan(
        &self,
        _table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }
}

struct Fold;

impl Collation for Fold {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase()))
    }
}

struct Resolver(RefCell<Vec<u64>>);

impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.0.borrow_mut().push(policy.host_key());
        assert_eq!(policy.host_key(), 1);
        Ok(&Fold)
    }
}

fn run(
    sql: &str,
    source: &Records,
    options: ExecutionOptions<'_>,
) -> Result<QueryResult, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), source)?.execute(source, &[], options)
}

fn kind(output: OutputType) -> Option<ScalarType> {
    match output {
        OutputType::Scalar { data_type, .. } => Some(data_type.scalar_type()),
        OutputType::Null | OutputType::Dynamic => None,
    }
}
