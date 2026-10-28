//! Independent final-provider membership and retained Text group observations.
use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::{Currency, Decimal, Value};
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, StringComparisonProfile},
    functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, PreparationOptions, QueryError, QueryLimits, Source, StreamError,
    prepare_with_options,
    source::{CollationResolver, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "progressive_text_group_values/membership.rs"]
mod membership;
#[path = "progressive_text_group_values/raw.rs"]
mod raw;
#[path = "progressive_text_group_values/scale.rs"]
mod scale;

struct Input {
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
    policies: Vec<Option<TextCollationId>>,
    allow_buffered: bool,
    active: Cell<bool>,
    eof: Cell<bool>,
}

impl Input {
    fn new(columns: &[(&str, ScalarType)], rows: Vec<Vec<Value>>) -> Self {
        Self {
            columns: columns
                .iter()
                .map(|(name, kind)| Column::new((*name).into(), *kind, true).unwrap())
                .collect(),
            rows,
            policies: vec![None; columns.len()],
            allow_buffered: false,
            active: Cell::new(false),
            eof: Cell::new(false),
        }
    }
}

impl Source for Input {
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
        assert!(
            self.allow_buffered,
            "progressive Text grouping must not snapshot input"
        );
        for row in &self.rows {
            visitor(row)?;
        }
        self.eof.set(true);
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
                visitor(ProjectedRow::select(row, indices)?)?;
            }
            self.eof.set(true);
            Ok(())
        })();
        self.active.set(false);
        result
    }
}

#[derive(Clone, Copy)]
enum Comparison {
    Ordinal,
    AsciiFold,
    AllEqual,
}

struct Order(Comparison);

impl Collation for Order {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        Ok(match self.0 {
            Comparison::Ordinal => left.cmp(right),
            Comparison::AsciiFold => left.to_ascii_uppercase().cmp(&right.to_ascii_uppercase()),
            Comparison::AllEqual => Ordering::Equal,
        })
    }
}

static ORDINAL: Order = Order(Comparison::Ordinal);
static FOLDED: Order = Order(Comparison::AsciiFold);
static ALL_EQUAL: Order = Order(Comparison::AllEqual);

struct Resolver<'a> {
    source: &'a Input,
    order: &'static dyn Collation,
    calls: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(
            self.source.eof.get(),
            "membership resolved before complete source EOF"
        );
        assert!(!self.source.active.get());
        assert_eq!(id.host_key(), 2);
        self.calls.set(self.calls.get() + 1);
        Ok(self.order)
    }
}

fn stream(
    sql: &str,
    source: &dyn Source,
    prepare_declared: bool,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, StreamError<()>> {
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
    plan.execute_progressive(source, &[], options, &mut |row| {
        rows.push(row.to_vec());
        Ok(())
    })?;
    Ok(rows)
}
