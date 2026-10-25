//! Independent numerical and resource controls for progressive global aggregates.
use std::cell::{Cell, RefCell};
use tablecraft_core::values::{Currency, Decimal, Double, LegacyDateTime, Single, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, StreamError, prepare,
    source::{ProjectedRow, Source},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "progressive_numeric_aggregate_values/boundaries.rs"]
mod boundaries;
#[path = "progressive_numeric_aggregate_values/values.rs"]
mod values;

struct Data {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    generated: Option<usize>,
    active: Cell<bool>,
    eof: Cell<bool>,
    visits: Cell<usize>,
    projections: RefCell<Vec<Vec<usize>>>,
    swallow: bool,
    fail_eof: bool,
}

impl Data {
    fn new(kind: ScalarType, values: Vec<Value>) -> Self {
        Self {
            columns: vec![Column::new("N".into(), kind, true).unwrap()],
            rows: values.into_iter().map(|value| vec![value]).collect(),
            generated: None,
            active: Cell::new(false),
            eof: Cell::new(false),
            visits: Cell::new(0),
            projections: RefCell::new(Vec::new()),
            swallow: false,
            fail_eof: false,
        }
    }

    fn generated(count: usize) -> Self {
        Self {
            generated: Some(count),
            ..Self::new(ScalarType::Int32, Vec::new())
        }
    }

    fn count(&self) -> usize {
        self.generated.unwrap_or(self.rows.len())
    }
}

impl Source for Data {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        assert!(!self.active.get());
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(
            self.generated.is_none(),
            "generated input must not materialize"
        );
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
        self.projections.borrow_mut().push(indices.to_vec());
        let result = (|| {
            for index in 0..self.count() {
                self.visits.set(self.visits.get() + 1);
                let generated;
                let row = if self.generated.is_some() {
                    generated = [Value::Int32(i32::try_from(index + 1).unwrap())];
                    generated.as_slice()
                } else {
                    self.rows[index].as_slice()
                };
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

fn stream(
    sql: &str,
    source: &Data,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut rows = Vec::new();
    let summary = plan.execute_progressive(source, &[], options, &mut |row| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        rows.push(row.to_vec());
        Ok(())
    })?;
    assert_eq!(summary.scanned_rows, source.count() as u64);
    assert_eq!(summary.emitted_rows, 1);
    Ok(rows)
}

fn double(value: f64) -> Value {
    Value::Double(Double::from_float(value))
}

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
}
