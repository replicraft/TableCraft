//! Fixed-key provenance and operational boundaries, independent of hash algorithms.
#[path = "exact_fixed_key_policies/boundaries.rs"]
mod boundaries;
#[path = "exact_fixed_key_policies/provenance.rs"]
mod provenance;

use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::{Currency, Decimal, Value};
use tablecraft_db::ProjectedRow;
use tablecraft_expressions::{
    EvalError, comparison::Collation, functions::conversions::text::NumberFormat,
};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, StreamError, prepare,
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
    buffered_scans: Cell<usize>,
    fail_eof: bool,
    swallow: bool,
}

impl Data {
    fn new(kind: ScalarType, values: Vec<(Value, i32, &'static str)>) -> Self {
        Self {
            columns: [
                ("D", kind),
                ("N", ScalarType::Int32),
                ("A", ScalarType::Text),
                ("B", ScalarType::Text),
            ]
            .map(|(name, kind)| Column::new(name.into(), kind, true).unwrap())
            .to_vec(),
            rows: values
                .into_iter()
                .map(|(key, number, text)| {
                    vec![
                        key,
                        Value::Int32(number),
                        Value::Text("unused".into()),
                        Value::Text(text.into()),
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
            buffered_scans: Cell::new(0),
            fail_eof: false,
            swallow: false,
        }
    }

    fn decimal_fixture() -> Self {
        Self::new(
            ScalarType::Decimal,
            vec![
                (Value::Null, 1, "15"),
                (decimal(100, 2), 2, "15"),
                (decimal(10, 1), 3, "15"),
                (Value::Null, 4, "15"),
            ],
        )
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

struct Order(bool);

impl Collation for Order {
    fn compare(&self, left: &str, right: &str) -> Result<Ordering, EvalError> {
        let order = left.cmp(right);
        Ok(if self.0 { order.reverse() } else { order })
    }
}

static ORDINAL: Order = Order(false);
static REVERSED: Order = Order(true);

struct Resolver<'a> {
    source: &'a Data,
    hits: Cell<usize>,
}

impl CollationResolver for Resolver<'_> {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        assert!(self.source.eof.get());
        assert!(!self.source.active.get());
        self.hits.set(self.hits.get() + 1);
        match id.host_key() {
            1 => Ok(&ORDINAL),
            2 => Ok(&REVERSED),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}

fn decimal(coefficient: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(coefficient, scale).unwrap())
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
    let mut rows = Vec::new();
    let buffered = source.buffered_scans.get();
    let result = plan.execute_progressive(source, &[], options, &mut |row| {
        assert!(source.eof.get());
        assert!(!source.active.get());
        rows.push(row.to_vec());
        Ok(())
    });
    assert_eq!(source.buffered_scans.get(), buffered);
    result?;
    Ok(rows)
}
