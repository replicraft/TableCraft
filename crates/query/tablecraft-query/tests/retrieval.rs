use std::{cell::Cell, cmp::Ordering};
use tablecraft_core::values::{Currency, Decimal, Double, Value};
use tablecraft_expressions::{EvalError, comparison::Collation};
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, QueryResult, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::{ParseLimits, parse_query};

struct Ordinal;
impl Collation for Ordinal {
    fn compare(&self, a: &str, b: &str) -> Result<Ordering, EvalError> {
        Ok(a.cmp(b))
    }
}
fn table(name: &str, columns: &[(&str, ScalarType)], rows: Vec<Vec<Value>>) -> MemoryTable {
    MemoryTable::new(
        name.into(),
        columns
            .iter()
            .map(|(n, t)| Column::new((*n).into(), *t, true).unwrap())
            .collect(),
        rows,
    )
    .unwrap()
}
fn source() -> MemorySource {
    MemorySource::new(vec![
        table(
            "Items",
            &[
                ("ID", ScalarType::Int64),
                ("Category", ScalarType::Int64),
                ("Price", ScalarType::Int64),
            ],
            vec![
                vec![Value::Int64(1), Value::Int64(10), Value::Int64(4)],
                vec![Value::Int64(2), Value::Int64(10), Value::Int64(8)],
                vec![Value::Int64(3), Value::Int64(20), Value::Null],
                vec![Value::Int64(4), Value::Null, Value::Int64(8)],
            ],
        ),
        table(
            "Categories",
            &[("ID", ScalarType::Int64)],
            vec![vec![Value::Int64(10)], vec![Value::Int64(30)]],
        ),
        table("Empty", &[("ID", ScalarType::Int64)], vec![]),
    ])
    .unwrap()
}
fn run(sql: &str, source: &dyn Source) -> Result<QueryResult, QueryError> {
    prepare(
        parse_query(sql, ParseLimits::default()).unwrap().query(),
        source,
    )?
    .execute(source, &[], ExecutionOptions::default())
}
fn ints(rows: &[&[Option<i64>]]) -> Vec<Vec<Value>> {
    rows.iter()
        .map(|r| {
            r.iter()
                .map(|n| n.map_or(Value::Null, Value::Int64))
                .collect()
        })
        .collect()
}
#[path = "retrieval/aggregation.rs"]
mod aggregation;
#[path = "retrieval/binding.rs"]
mod binding;
#[path = "retrieval/boundaries.rs"]
mod boundaries;
#[path = "retrieval/selection.rs"]
mod selection;
#[path = "retrieval/subqueries.rs"]
mod subqueries;
