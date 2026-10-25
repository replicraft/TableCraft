//! Independent values and populations across static stateful progressive stages.
use std::cell::Cell;
use tablecraft_core::values::{Currency, Decimal, Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, Source, StreamError, StreamSummary,
    prepare_with_options, source::ProjectedRow,
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

#[path = "progressive_stateful_stage_values/arithmetic.rs"]
mod arithmetic;
#[path = "progressive_stateful_stage_values/composition.rs"]
mod composition;
#[path = "progressive_stateful_stage_values/retained.rs"]
mod retained;

struct Input {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    definitions: Vec<(String, Query)>,
    active: Cell<bool>,
    eof: Cell<bool>,
    visits: Cell<usize>,
    cursors: Cell<usize>,
}

impl Input {
    fn new(columns: &[(&str, ScalarType)], rows: Vec<Vec<Value>>) -> Self {
        Self {
            columns: columns
                .iter()
                .map(|(name, kind)| Column::new((*name).into(), *kind, true).unwrap())
                .collect(),
            rows,
            definitions: Vec::new(),
            active: Cell::new(false),
            eof: Cell::new(false),
            visits: Cell::new(0),
            cursors: Cell::new(0),
        }
    }

    fn fixture() -> Self {
        Self::new(
            &[
                ("Id", ScalarType::Int32),
                ("G", ScalarType::Int16),
                ("Score", ScalarType::Int32),
            ],
            [Some(2), Some(1), Some(2), Some(1), None, None]
                .into_iter()
                .enumerate()
                .map(|(index, group)| {
                    let id = index as i32 + 1;
                    vec![
                        Value::Int32(id),
                        group.map_or(Value::Null, Value::Int16),
                        if id == 3 {
                            Value::Null
                        } else {
                            Value::Int32(id)
                        },
                    ]
                })
                .collect(),
        )
    }

    fn define(&mut self, name: &str, sql: &str) {
        self.definitions.push((
            name.into(),
            parse_query(sql, Default::default()).unwrap().into_query(),
        ));
    }
}

impl Source for Input {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        assert!(!self.active.get());
        Ok(self
            .definitions
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        assert!(!self.active.get());
        if table != "T" {
            return Err(QueryError::UnknownName);
        }
        Ok(&self.columns)
    }

    fn scan(
        &self,
        _table: &str,
        _visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("stateful progressive stages must not materialize a source snapshot");
    }

    fn scan_projected_progressive(
        &self,
        table: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert_eq!(table, "T");
        assert!(!self.active.replace(true));
        self.eof.set(false);
        self.cursors.set(self.cursors.get() + 1);
        let result = (|| {
            for row in &self.rows {
                self.visits.set(self.visits.get() + 1);
                visitor(ProjectedRow::select(row, indices)?)?;
            }
            self.eof.set(true);
            Ok(())
        })();
        self.active.set(false);
        result
    }
}

fn run(
    sql: &str,
    source: &Input,
    parameters: &[(&str, Value)],
    prepare_declared: bool,
    declared_integral: bool,
) -> Result<(StreamSummary, Vec<Vec<Value>>), StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare_with_options(
        syntax.query(),
        source,
        PreparationOptions {
            declared_arithmetic: prepare_declared,
            ..Default::default()
        },
    )?;
    let mut rows = Vec::new();
    let summary = plan.execute_progressive(
        source,
        parameters,
        ExecutionOptions {
            declared_integral,
            ..Default::default()
        },
        &mut |row| {
            rows.push(row.to_vec());
            Ok(())
        },
    )?;
    Ok((summary, rows))
}
