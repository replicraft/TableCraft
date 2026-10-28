//! Independent HAVING suppression, retained-state and checked-EOF controls.
#[path = "progressive_having_values/phases.rs"]
mod phases;
#[path = "progressive_having_values/retained.rs"]
mod retained;
#[path = "progressive_having_values/suppression.rs"]
mod suppression;

use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::{Double, Value};
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, QueryError, StreamError, prepare,
    source::{CollationResolver, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

struct Data {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    active: Cell<bool>,
    eof: Cell<bool>,
    visits: Cell<usize>,
    buffered_scans: Cell<usize>,
    fail_eof: bool,
}

impl Data {
    fn new(values: &[(Option<i32>, i32)]) -> Self {
        Self {
            columns: [
                ("K", ScalarType::Int32),
                ("N", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
            ]
            .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
            .to_vec(),
            rows: values
                .iter()
                .map(|(key, value)| {
                    vec![
                        key.map_or(Value::Null, Value::Int32),
                        Value::Int32(*value),
                        Value::Text("unused".into()),
                        Value::Text("15".into()),
                    ]
                })
                .collect(),
            policies: vec![
                None,
                None,
                Some(TextCollationId::from_host(1)),
                Some(TextCollationId::from_host(2)),
            ],
            active: Cell::new(false),
            eof: Cell::new(false),
            visits: Cell::new(0),
            buffered_scans: Cell::new(0),
            fail_eof: false,
        }
    }

    fn fixture() -> Self {
        Self::new(&[
            (Some(2), 2),
            (Some(1), 1),
            (Some(2), 5),
            (None, 9),
            (Some(3), 1),
            (Some(1), 2),
        ])
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
        self.buffered_scans.set(self.buffered_scans.get() + 1);
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
                self.visits.set(self.visits.get() + 1);
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
    let buffered_before = source.buffered_scans.get();
    let result = plan.execute_progressive(source, &[], options, &mut |row| {
        assert!(source.eof.get(), "HAVING output before checked EOF");
        assert!(!source.active.get());
        output.push(row.to_vec());
        Ok(())
    });
    assert_eq!(source.buffered_scans.get(), buffered_before);
    result?;
    Ok(output)
}
