//! COUNT operands retain ordinary nullness and eager validation in progressive scans.
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use tablecraft_core::values::{Decimal, Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, OutputType, QueryError, QueryLimits, StreamError, prepare,
    source::{CollationResolver, ProjectedRow, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType, TypeDescriptor};
use tablecraft_sql::parse_query;

#[path = "progressive_count_operands/boundaries.rs"]
mod boundaries;
#[path = "progressive_count_operands/values.rs"]
mod values;

struct Generated {
    columns: Vec<Column>,
    policies: Vec<Option<TextCollationId>>,
    count: usize,
    projections: RefCell<Vec<Vec<usize>>>,
    visits: Cell<usize>,
    active: Cell<bool>,
    eof: Cell<bool>,
    buffered_calls: Cell<usize>,
    buffered_allowed: bool,
    swallow: bool,
    fail_eof: bool,
}

impl Generated {
    fn new(count: usize) -> Self {
        Self {
            columns: vec![
                Column::new("N".into(), ScalarType::Int32, false).unwrap(),
                Column::new("A".into(), ScalarType::Text, true).unwrap(),
            ],
            policies: vec![None, Some(TextCollationId::from_host(999))],
            count,
            projections: RefCell::new(Vec::new()),
            visits: Cell::new(0),
            active: Cell::new(false),
            eof: Cell::new(false),
            buffered_calls: Cell::new(0),
            buffered_allowed: false,
            swallow: false,
            fail_eof: false,
        }
    }

    fn row(number: usize, text_needed: bool) -> [Value; 2] {
        let text = if text_needed {
            match number % 3 {
                1 => Value::Text("a".into()),
                2 => Value::Null,
                _ => Value::Text("b".into()),
            }
        } else {
            Value::Null
        };
        [Value::Int32(i32::try_from(number).unwrap()), text]
    }
}

impl Source for Generated {
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
        self.buffered_calls.set(self.buffered_calls.get() + 1);
        assert!(self.buffered_allowed);
        for number in 1..=self.count {
            visitor(&Self::row(number, true))?;
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
                self.visits.set(self.visits.get() + 1);
                let row = Self::row(number, indices.contains(&1));
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

struct Ordinal;

impl Collation for Ordinal {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(left.cmp(right))
    }
}

struct Resolver(Cell<usize>);

impl CollationResolver for Resolver {
    fn resolve(&self, policy: TextCollationId) -> Result<&dyn Collation, QueryError> {
        self.0.set(self.0.get() + 1);
        assert_eq!(policy.host_key(), 1);
        Ok(&Ordinal)
    }
}

fn rows(sql: &str, source: &Generated, options: ExecutionOptions<'_>) -> Vec<Vec<Value>> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let plan = prepare(syntax.query(), source).unwrap();
    let mut output = Vec::new();
    let summary = plan
        .execute_progressive(source, &[], options, &mut |row| {
            assert!(source.eof.get());
            assert!(!source.active.get());
            output.push(row.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(summary.emitted_rows, 1);
    assert_eq!(summary.scanned_rows, source.count as u64);
    output
}
