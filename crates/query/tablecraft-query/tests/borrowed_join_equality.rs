//! Bare integral equijoins retain order without materializing rejected pairs.
use std::cell::Cell;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, Source, prepare,
    source::{MemorySource, MemoryTable},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "borrowed_join_equality/boundaries.rs"]
mod boundaries;
#[path = "borrowed_join_equality/semantics.rs"]
mod semantics;

struct Tables {
    inner: MemorySource,
    scans: Cell<usize>,
    fail_right: bool,
    wrong_right: bool,
}

impl Source for Tables {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.inner.columns(table)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scans.set(self.scans.get() + 1);
        if table == "R" && self.wrong_right {
            visitor(&[
                Value::Text("invalid".into()),
                Value::Int32(20),
                Value::Text("r".into()),
            ])?;
            return Ok(());
        }
        self.inner.scan(table, visitor)?;
        if table == "R" && self.fail_right {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}

fn carrier(kind: ScalarType, value: i32) -> Value {
    match kind {
        ScalarType::Boolean => Value::Boolean(value != 0),
        ScalarType::Byte => Value::Byte(u8::try_from(value).unwrap()),
        ScalarType::Int16 => Value::Int16(i16::try_from(value).unwrap()),
        ScalarType::Int32 => Value::Int32(value),
        ScalarType::Int64 => Value::Int64(i64::from(value)),
        _ => panic!("test requires a proven integral carrier"),
    }
}

fn table(name: &str, kind: ScalarType, keys: &[Option<i32>]) -> MemoryTable {
    let rows = keys
        .iter()
        .enumerate()
        .map(|(index, key)| {
            vec![
                key.map_or(Value::Null, |key| carrier(kind, key)),
                Value::Int32(i32::try_from(index).unwrap()),
                Value::Text(format!("{name}{index}")),
            ]
        })
        .collect();
    MemoryTable::new(
        name.into(),
        vec![
            Column::new("K".into(), kind, true).unwrap(),
            Column::new("Id".into(), ScalarType::Int32, false).unwrap(),
            Column::new("Payload".into(), ScalarType::Text, false).unwrap(),
        ],
        rows,
    )
    .unwrap()
}

fn tables(kind: ScalarType, left: &[Option<i32>], right: &[Option<i32>]) -> Tables {
    Tables {
        inner: MemorySource::new(vec![table("L", kind, left), table("R", kind, right)]).unwrap(),
        scans: Cell::new(0),
        fail_right: false,
        wrong_right: false,
    }
}

fn execute(
    sql: &str,
    source: &dyn Source,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    let prepared = prepare(syntax.query(), source)?;
    Ok(prepared.execute(source, &[], options)?.rows().to_vec())
}
