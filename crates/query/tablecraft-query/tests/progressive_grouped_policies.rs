//! Independent retained-cell and provider boundaries for progressive grouping.
#[path = "progressive_grouped_policies/cohorts.rs"]
mod cohorts;
#[path = "progressive_grouped_policies/retained.rs"]
mod retained;

use std::cell::Cell;
use std::cmp::Ordering;
use tablecraft_core::values::{Double, Value};
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, QueryError, StreamError, prepare,
    source::{CollationResolver, ProjectedRow, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Data {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    eof: Cell<bool>,
    active: Cell<bool>,
    fail_eof: bool,
}

impl Data {
    fn new(rows: &[(i32, i32, &str)]) -> Self {
        Self {
            columns: [
                ("G", ScalarType::Int32),
                ("Id", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
            ]
            .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
            .to_vec(),
            rows: rows
                .iter()
                .map(|(group, id, text)| {
                    vec![
                        Value::Int32(*group),
                        Value::Int32(*id),
                        Value::Text("unused".into()),
                        Value::Text((*text).into()),
                    ]
                })
                .collect(),
            policies: vec![
                None,
                None,
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
            ],
            eof: Cell::new(false),
            active: Cell::new(false),
            fail_eof: false,
        }
    }
}

impl Source for Data {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        assert!(
            !self.active.get(),
            "metadata queried inside exclusive cursor"
        );
        Ok(&self.columns)
    }

    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        assert!(!self.active.get());
        Ok(&self.policies)
    }

    fn scan(
        &self,
        _table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        for row in &self.rows {
            visitor(row)?;
        }
        Ok(())
    }

    fn scan_projected_progressive(
        &self,
        _table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(!self.active.replace(true));
        self.eof.set(false);
        let result = (|| {
            for row in &self.rows {
                visitor(ProjectedRow::select(row, indices)?)?;
            }
            if self.fail_eof {
                return Err(QueryError::Source);
            }
            self.eof.set(true);
            Ok(())
        })();
        self.active.set(false);
        result
    }
}

struct Order(bool);

impl Collation for Order {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        let comparison = left.cmp(right);
        Ok(if self.0 {
            comparison.reverse()
        } else {
            comparison
        })
    }
}

static ORDINAL: Order = Order(false);
static REVERSED: Order = Order(true);

struct Resolver<'a> {
    source: &'a Data,
    hits: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(
            self.source.eof.get(),
            "resolver activated before checked EOF"
        );
        assert!(!self.source.active.get());
        self.hits.set(self.hits.get() + 1);
        match id.host_key() {
            1 => Ok(&ORDINAL),
            2 => Ok(&REVERSED),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

fn options(declared: bool) -> ExecutionOptions<'static> {
    ExecutionOptions {
        declared_integral: declared,
        number_format: Some(NumberFormat::Invariant),
        ..Default::default()
    }
}

fn stream(
    source: &Data,
    sql: &str,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut output = Vec::new();
    plan.execute_progressive(source, &[], options, &mut |row| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        output.push(row.to_vec());
        Ok(())
    })?;
    Ok(output)
}
