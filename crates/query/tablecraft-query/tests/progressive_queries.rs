use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_query::source::{ProjectedRow, Source, TextCollationId};
use tablecraft_query::{ExecutionOptions, QueryError, StreamError, prepare};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "progressive_queries/boundaries.rs"]
mod boundaries;
#[path = "progressive_queries/counts.rs"]
mod counts;
#[path = "progressive_queries/execution.rs"]
mod execution;
#[path = "progressive_queries/numeric.rs"]
mod numeric;
#[path = "progressive_queries/top.rs"]
mod top;
#[path = "progressive_queries/union.rs"]
mod union;

struct Rows {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    scans: Cell<usize>,
    visits: Cell<usize>,
    swallow: bool,
    eof_error: bool,
    changed: Cell<bool>,
    policies: Vec<Option<TextCollationId>>,
}

impl Rows {
    fn new(values: impl IntoIterator<Item = i32>) -> Self {
        Self {
            columns: vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            rows: values
                .into_iter()
                .map(|value| vec![Value::Int32(value)])
                .collect(),
            scans: Cell::new(0),
            visits: Cell::new(0),
            swallow: false,
            eof_error: false,
            changed: Cell::new(false),
            policies: vec![],
        }
    }
}

impl Source for Rows {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        if self.changed.get() {
            return Err(QueryError::SchemaChanged);
        }
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

    fn scan_projected_progressive(
        &self,
        _table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        for row in &self.rows {
            self.visits.set(self.visits.get() + 1);
            let result = visitor(ProjectedRow::select(row, indices)?);
            if !self.swallow {
                result?;
            }
        }
        if self.eof_error {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}
