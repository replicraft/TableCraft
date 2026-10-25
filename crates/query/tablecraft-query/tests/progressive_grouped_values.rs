//! Independent generated rows and arithmetic models for progressive grouping.
use std::cell::{Cell, RefCell};
use tablecraft_core::values::{Decimal, Double, Value};
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, StreamError, prepare,
    source::{ProjectedRow, ScanConstraint, Source, validate_projection},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::parse_query;

#[path = "progressive_grouped_values/arithmetic.rs"]
mod arithmetic;
#[path = "progressive_grouped_values/boundaries.rs"]
mod boundaries;
#[path = "progressive_grouped_values/decimal.rs"]
mod decimal;
#[path = "progressive_grouped_values/scale.rs"]
mod scale;

enum Population {
    Generated { first: i32, count: usize },
    Stored(Vec<Vec<Value>>),
}

struct Inputs {
    columns: Vec<Column>,
    population: Population,
    group_kind: ScalarType,
    allow_buffered: bool,
    buffered_calls: Cell<usize>,
    scans: RefCell<Vec<String>>,
    projections: RefCell<Vec<Vec<usize>>>,
    visits: Cell<usize>,
    active: Cell<bool>,
    eof: Cell<bool>,
    fail_eof: bool,
    swallow: bool,
    corrupt_after: Option<usize>,
}

impl Inputs {
    fn generated(first: i32, count: usize) -> Self {
        Self {
            columns: vec![
                Column::new("Unused".into(), ScalarType::Int32, false).unwrap(),
                Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
                Column::new("G".into(), ScalarType::Int16, true).unwrap(),
                Column::new("Score".into(), ScalarType::Int32, true).unwrap(),
                Column::with_type(
                    "Amount".into(),
                    TypeDescriptor::decimal(28, 4).unwrap(),
                    true,
                )
                .unwrap(),
            ],
            population: Population::Generated { first, count },
            group_kind: ScalarType::Int16,
            allow_buffered: false,
            buffered_calls: Cell::new(0),
            scans: RefCell::new(Vec::new()),
            projections: RefCell::new(Vec::new()),
            visits: Cell::new(0),
            active: Cell::new(false),
            eof: Cell::new(false),
            fail_eof: false,
            swallow: false,
            corrupt_after: None,
        }
    }

    fn row(&self, id: i32) -> Vec<Value> {
        let group = if id % 7 == 0 {
            Value::Null
        } else {
            key(self.group_kind, i128::from(id % 2))
        };
        let score = if id % 11 == 0 {
            Value::Null
        } else {
            Value::Int32(id % 31 - 15)
        };
        let amount = if id % 11 == 0 {
            Value::Null
        } else {
            Value::Decimal(Decimal::new(i128::from(id % 7 - 3), 4).unwrap())
        };
        vec![
            Value::Text("unselected malformed Int32".into()),
            Value::Int32(id),
            group,
            score,
            amount,
        ]
    }

    fn visit(
        &self,
        name: &str,
        mut visitor: impl FnMut(usize, &[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        if name == "Later" {
            for index in 0..2 {
                visitor(index, &self.row(101 + index as i32))?;
            }
            return Ok(());
        }
        if name != "T" {
            return Err(QueryError::Source);
        }
        match &self.population {
            Population::Generated { first, count } => {
                for index in 0..*count {
                    let row = self.row(first + i32::try_from(index).unwrap());
                    visitor(index, &row)?;
                }
            }
            Population::Stored(rows) => {
                for (index, row) in rows.iter().enumerate() {
                    visitor(index, row)?;
                }
            }
        }
        Ok(())
    }
}

impl Source for Inputs {
    fn columns(&self, name: &str) -> Result<&[Column], QueryError> {
        assert!(
            !self.active.get(),
            "metadata cannot reenter an exclusive cursor"
        );
        if matches!(name, "T" | "Later") {
            Ok(&self.columns)
        } else {
            Err(QueryError::Source)
        }
    }

    fn scan(
        &self,
        _: &str,
        _: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        panic!("only explicitly projected capabilities are supplied")
    }

    fn scan_projected(
        &self,
        name: &str,
        indices: &[usize],
        _: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(
            self.allow_buffered,
            "progressive grouping must not request a snapshot"
        );
        self.buffered_calls.set(self.buffered_calls.get() + 1);
        self.visit(name, |_, row| visitor(ProjectedRow::select(row, indices)?))
    }

    fn scan_projected_progressive(
        &self,
        name: &str,
        indices: &[usize],
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        assert!(!self.active.get());
        validate_projection(indices, self.columns.len())?;
        self.active.set(true);
        self.eof.set(false);
        self.scans.borrow_mut().push(name.into());
        self.projections.borrow_mut().push(indices.to_vec());
        let result = (|| {
            self.visit(name, |index, row| {
                self.visits.set(self.visits.get() + 1);
                let mut bad;
                let row = if self.corrupt_after == Some(index) {
                    bad = row.to_vec();
                    bad[1] = Value::Text("bad selected Id".into());
                    &bad
                } else {
                    row
                };
                let result = visitor(ProjectedRow::select(row, indices)?);
                if self.swallow { Ok(()) } else { result }
            })?;
            if self.fail_eof && name == "T" {
                return Err(QueryError::Source);
            }
            self.eof.set(true);
            Ok(())
        })();
        self.active.set(false);
        result
    }
}

fn key(kind: ScalarType, value: i128) -> Value {
    match kind {
        ScalarType::Boolean => Value::Boolean(value != 0),
        ScalarType::Byte => Value::Byte(u8::try_from(value).unwrap()),
        ScalarType::Int16 => Value::Int16(i16::try_from(value).unwrap()),
        ScalarType::Int32 => Value::Int32(i32::try_from(value).unwrap()),
        ScalarType::Int64 => Value::Int64(i64::try_from(value).unwrap()),
        _ => panic!("model uses only exact integral carriers"),
    }
}

fn stream(
    sql: &str,
    source: &Inputs,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, StreamError<()>> {
    let query = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(query.query(), source).unwrap();
    let mut output = Vec::new();
    plan.execute_progressive(source, &[], options, &mut |row| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        output.push(row.to_vec());
        Ok(())
    })?;
    Ok(output)
}

fn remainder(value: i32, divisor: i32) -> i32 {
    let magnitude = value.abs() / divisor;
    let quotient = if value < 0 { -magnitude } else { magnitude };
    value - quotient * divisor
}
