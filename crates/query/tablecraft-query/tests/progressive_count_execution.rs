//! Generated progressive inputs expose COUNT behavior without a parent snapshot.
use std::cell::{Cell, RefCell};
use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, StreamError, prepare,
    source::{ProjectedRow, Source},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "progressive_count_execution/boundaries.rs"]
mod boundaries;
#[path = "progressive_count_execution/values.rs"]
mod values;

struct Generated {
    columns: Vec<Column>,
    count: usize,
    projections: RefCell<Vec<Vec<usize>>>,
    visits: Cell<usize>,
    active: Cell<bool>,
    eof: Cell<bool>,
    buffered_calls: Cell<usize>,
    buffered_allowed: bool,
    swallow: bool,
    fail_at: Option<usize>,
    fail_eof: bool,
}

impl Generated {
    fn new(count: usize) -> Self {
        Self {
            columns: vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
            count,
            projections: RefCell::new(Vec::new()),
            visits: Cell::new(0),
            active: Cell::new(false),
            eof: Cell::new(false),
            buffered_calls: Cell::new(0),
            buffered_allowed: false,
            swallow: false,
            fail_at: None,
            fail_eof: false,
        }
    }
}

impl Source for Generated {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        assert!(
            !self.active.get(),
            "metadata must precede the exclusive cursor"
        );
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.buffered_calls.set(self.buffered_calls.get() + 1);
        assert!(
            self.buffered_allowed,
            "progressive COUNT must not snapshot its input"
        );
        for number in 1..=self.count {
            let value = i32::try_from(number).unwrap();
            visitor(&[Value::Int32(value)])?;
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
        self.projections.borrow_mut().push(indices.to_vec());
        let result = (|| {
            for number in 1..=self.count {
                if self.fail_at == Some(number) {
                    return Err(QueryError::Source);
                }
                self.visits.set(self.visits.get() + 1);
                let value = i32::try_from(number).unwrap();
                let row = [Value::Int32(value)];
                let result = visitor(ProjectedRow::select(&row, indices)?);
                if !self.swallow {
                    result?;
                }
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

fn wide_options() -> ExecutionOptions<'static> {
    ExecutionOptions {
        limits: QueryLimits {
            rows: 300_000,
            steps: 10_000_000,
            ..Default::default()
        },
        ..Default::default()
    }
}
