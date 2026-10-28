//! DISTINCT uses exact SQL keys without materializing its progressive source.
#[path = "progressive_distinct/boundaries.rs"]
mod boundaries;
#[path = "progressive_distinct/values.rs"]
mod values;

use std::cell::Cell;
use tablecraft_core::values::{Currency, Decimal, Double, Value};
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, QueryLimits, StreamError, prepare,
    prepare_with_options, source::Source,
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Data {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    generated: Option<usize>,
    active: Cell<bool>,
    eof: Cell<bool>,
    scans: Cell<usize>,
    buffered: Cell<usize>,
    fail_eof: bool,
    swallow: bool,
}

impl Data {
    fn new(kind: ScalarType, values: Vec<Value>) -> Self {
        Self {
            columns: vec![Column::new("N".into(), kind, true).unwrap()],
            rows: values.into_iter().map(|value| vec![value]).collect(),
            generated: None,
            active: Cell::new(false),
            eof: Cell::new(false),
            scans: Cell::new(0),
            buffered: Cell::new(0),
            fail_eof: false,
            swallow: false,
        }
    }

    fn each(
        &self,
        mut visit: impl FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if let Some(count) = self.generated {
            for number in 0..count {
                visit(&[Value::Int32(i32::try_from(number).unwrap())])?;
            }
        } else {
            for row in &self.rows {
                visit(row)?;
            }
        }
        Ok(())
    }
}

impl Source for Data {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        assert!(!self.active.get(), "metadata must stay outside the cursor");
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.buffered.set(self.buffered.get() + 1);
        assert!(
            self.generated.is_none(),
            "generated rows must not be snapshotted"
        );
        self.each(visitor)
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
        let result = self.each(|row| {
            let result = visitor(ProjectedRow::select(row, indices)?);
            if self.swallow { Ok(()) } else { result }
        });
        self.active.set(false);
        result?;
        if self.fail_eof {
            return Err(QueryError::Source);
        }
        self.eof.set(true);
        Ok(())
    }
}

fn stream(
    source: &Data,
    sql: &str,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(parsed.query(), source).unwrap();
    let before = source.buffered.get();
    let first_scan = source.scans.get() + 1;
    let mut rows = Vec::new();
    let result = plan.execute_progressive(source, &[], options, &mut |row| {
        if source.scans.get() == first_scan {
            assert!(source.eof.get());
            assert!(!source.active.get());
        }
        rows.push(row.to_vec());
        Ok(())
    });
    assert_eq!(source.buffered.get(), before);
    result?;
    Ok(rows)
}

fn buffered(source: &Data, sql: &str, options: ExecutionOptions<'_>) -> Vec<Vec<Value>> {
    let parsed = parse_query(sql, Default::default()).unwrap();
    prepare(parsed.query(), source)
        .unwrap()
        .execute(source, &[], options)
        .unwrap()
        .rows()
        .to_vec()
}
