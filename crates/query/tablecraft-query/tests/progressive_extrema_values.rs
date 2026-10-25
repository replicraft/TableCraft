//! Independent retained-value and checked-EOF controls for progressive extrema.
#[path = "progressive_extrema_values/boundaries.rs"]
mod boundaries;
#[path = "progressive_extrema_values/edges.rs"]
mod edges;
#[path = "progressive_extrema_values/policies.rs"]
mod policies;
#[path = "progressive_extrema_values/values.rs"]
mod values;

use std::cell::{Cell, RefCell};
use tablecraft_core::values::{
    Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single, Value,
};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, StreamError, prepare,
    source::{ProjectedRow, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Data {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    eof: Cell<bool>,
    active: Cell<bool>,
    visits: Cell<usize>,
    scans: Cell<usize>,
    projections: RefCell<Vec<Vec<usize>>>,
    swallow: bool,
    fail_eof: bool,
}

impl Data {
    fn new(kind: ScalarType, values: Vec<Value>) -> Self {
        Self {
            columns: vec![Column::new("N".into(), kind, true).unwrap()],
            rows: values.into_iter().map(|value| vec![value]).collect(),
            policies: vec![None],
            eof: Cell::new(false),
            active: Cell::new(false),
            visits: Cell::new(0),
            scans: Cell::new(0),
            projections: RefCell::new(Vec::new()),
            swallow: false,
            fail_eof: false,
        }
    }
}

impl Source for Data {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        assert!(!self.active.get());
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
        self.scans.set(self.scans.get() + 1);
        self.projections.borrow_mut().push(indices.to_vec());
        let result = (|| {
            for row in &self.rows {
                self.visits.set(self.visits.get() + 1);
                let result = visitor(ProjectedRow::select(row, indices)?);
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

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}
