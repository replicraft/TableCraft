//! Independent root-local models for static named and derived progressive stages.
use std::cell::{Cell, RefCell};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, QueryLimits, Source, StreamError,
    StreamSummary, prepare, prepare_with_options,
    source::{ProjectedRow, validate_projection},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{parse_query, query::Query};

#[path = "progressive_derived_values/boundaries.rs"]
mod boundaries;
#[path = "progressive_derived_values/composition.rs"]
mod composition;
#[path = "progressive_derived_values/scale.rs"]
mod scale;

struct Inputs {
    columns: Vec<Column>,
    definitions: Vec<(String, Query)>,
    count: usize,
    scans: RefCell<Vec<String>>,
    projections: RefCell<Vec<Vec<usize>>>,
    visits: Cell<usize>,
    active: Cell<bool>,
    eof: Cell<bool>,
    fail_eof: bool,
    swallow: bool,
    corrupt: Option<usize>,
    drift_after_data: bool,
    changed: Cell<bool>,
}

impl Inputs {
    fn new(count: usize, definitions: &[(&str, &str)]) -> Self {
        Self {
            columns: vec![
                Column::new("Unused".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
                Column::new("Label".into(), ScalarType::Text, false).unwrap(),
            ],
            definitions: definitions
                .iter()
                .map(|(name, sql)| {
                    (
                        (*name).into(),
                        parse_query(sql, Default::default()).unwrap().into_query(),
                    )
                })
                .collect(),
            count,
            scans: RefCell::new(Vec::new()),
            projections: RefCell::new(Vec::new()),
            visits: Cell::new(0),
            active: Cell::new(false),
            eof: Cell::new(false),
            fail_eof: false,
            swallow: false,
            corrupt: None,
            drift_after_data: false,
            changed: Cell::new(false),
        }
    }

    fn row(id: i32) -> [Value; 4] {
        let score = if id % 5 == 0 {
            Value::Null
        } else {
            Value::Int32(id * 3)
        };
        [
            Value::Text("unselected malformed Int32".into()),
            Value::Int32(id),
            score,
            Value::Text(format!("row:{id}")),
        ]
    }
}

impl Source for Inputs {
    fn query(&self, name: &str) -> Result<Option<&Query>, QueryError> {
        assert!(
            !self.active.get(),
            "definitions are metadata, not cursor work"
        );
        Ok(self
            .definitions
            .iter()
            .find(|(known, _)| known.eq_ignore_ascii_case(name))
            .map(|(_, query)| query))
    }

    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        assert!(
            !self.active.get(),
            "metadata must not reenter an exclusive cursor"
        );
        if name == "Other" && self.changed.get() {
            return Err(QueryError::SchemaChanged);
        }
        match name {
            "Data" | "Other" => Ok(&self.columns),
            _ => Err(QueryError::Source),
        }
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("the source does not supply a complete snapshot")
    }

    fn scan_projected_progressive(
        &self,
        name: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(!self.active.get());
        if !matches!(name, "Data" | "Other") {
            return Err(QueryError::Source);
        }
        validate_projection(indices, self.columns.len())?;
        self.active.set(true);
        self.eof.set(false);
        self.scans.borrow_mut().push(name.into());
        self.projections.borrow_mut().push(indices.to_vec());
        let result = (|| {
            let count = if name == "Data" { self.count } else { 2 };
            for ordinal in 0..count {
                self.visits.set(self.visits.get() + 1);
                let mut row = Self::row(i32::try_from(ordinal + 1).unwrap());
                if self.corrupt == Some(ordinal) {
                    row[1] = Value::Text("bad selected Id".into());
                }
                let result = visitor(ProjectedRow::select(&row, indices)?);
                if !self.swallow {
                    result?;
                }
            }
            if name == "Data" && self.fail_eof {
                return Err(QueryError::Source);
            }
            if name == "Data" && self.drift_after_data {
                self.changed.set(true);
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
    source: &Inputs,
    parameters: &[(&str, Value)],
    options: ExecutionOptions<'_>,
) -> Result<(StreamSummary, Vec<Vec<Value>>), StreamError<()>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source)?;
    let mut rows = Vec::new();
    let summary = plan.execute_progressive(source, parameters, options, &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    })?;
    Ok((summary, rows))
}
