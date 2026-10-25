//! Independent checks of publication after whole-UNION carrier promotion.
use std::cell::{Cell, RefCell};
use tablecraft_core::values::{Decimal, Double, Value};
use tablecraft_expressions::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_query::source::{MemorySource, MemoryTable, ProjectedRow, Source, TextCollationId};
use tablecraft_query::{ExecutionOptions, QueryError, QueryLimits, StreamError, prepare};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "progressive_union_types/boundaries.rs"]
mod boundaries;
#[path = "progressive_union_types/promotion.rs"]
mod promotion;

struct Tables {
    data: MemorySource,
    scans: RefCell<Vec<String>>,
    visits: Cell<usize>,
    swallow: bool,
    fail_eof: Option<&'static str>,
    changed_b: Cell<bool>,
    change_after_a: bool,
    policies: Vec<Option<TextCollationId>>,
}

impl Tables {
    fn new(values: &[i32]) -> Self {
        let tables = ["A", "B"]
            .into_iter()
            .map(|name| {
                MemoryTable::new(
                    name.into(),
                    vec![Column::new("N".into(), ScalarType::Int32, false).unwrap()],
                    values
                        .iter()
                        .map(|value| vec![Value::Int32(*value)])
                        .collect(),
                )
                .unwrap()
            })
            .collect();

        Self {
            data: MemorySource::new(tables).unwrap(),
            scans: RefCell::new(Vec::new()),
            visits: Cell::new(0),
            swallow: false,
            fail_eof: None,
            changed_b: Cell::new(false),
            change_after_a: false,
            policies: vec![],
        }
    }
}

impl Source for Tables {
    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.policies)
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if table == "B" && self.changed_b.get() {
            return Err(QueryError::SchemaChanged);
        }
        self.data.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.data.scan(table, visitor)
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.borrow_mut().push(table.into());
        self.data
            .scan_projected_progressive(table, indices, &mut |row| {
                self.visits.set(self.visits.get() + 1);
                let result = visitor(row);
                if self.swallow {
                    return Ok(());
                }

                result
            })?;

        if table == "A" && self.change_after_a {
            self.changed_b.set(true);
        }

        if self.fail_eof == Some(table) {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}

fn stream(sql: &str, source: &Tables, options: ExecutionOptions<'_>) -> Vec<Vec<Value>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let buffered = plan.execute(source, &[], options).unwrap();
    let mut rows = Vec::new();
    let summary = plan
        .execute_progressive(source, &[], options, &mut |row| {
            rows.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();

    assert_eq!(rows, buffered.rows(), "{sql}");
    assert_eq!(summary.emitted_rows, rows.len() as u64, "{sql}");
    rows
}
