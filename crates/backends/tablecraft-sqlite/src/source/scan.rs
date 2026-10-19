use super::SqliteSource;
use crate::{SqliteError, work::Work};
use rusqlite::{Connection, types::ValueRef};
use std::borrow::Borrow;
use tablecraft_core::{
    limits::Resource,
    values::{Double, Value},
};
use tablecraft_query::{QueryError, Source};
use tablecraft_schema::{Column, ScalarType, validate_row};

impl<C: Borrow<Connection>> SqliteSource<C> {
    pub(super) fn scan_table(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let _span = tracing::debug_span!("sqlite.table.scan").entered();
        let entry = self.entry(table)?;
        let columns = self.columns(table)?;
        let sql = selection(entry.name.as_str(), columns, &self.work)?;
        self.work.check()?;
        let mut statement = self.work.sql(self.connection.borrow().prepare(&sql))?;
        let mut rows = self.work.sql(statement.query([]))?;

        while let Some(row) = self.work.sql(rows.next())? {
            self.work.check()?;
            self.work.charge(Resource::Rows, 1)?;
            let mut values = Vec::new();
            self.work.reserve(&mut values, columns.len())?;
            for (position, column) in columns.iter().enumerate() {
                self.work.check()?;
                let raw = self.work.sql(row.get_ref(position))?;
                values.push(decode(raw, column.data_type(), &self.work)?);
            }
            validate_row(columns, &values)?;
            // Preserve the visitor's exact failure and stop before stepping again.
            visitor(&values)?;
        }
        self.work.check()?;
        Ok(())
    }
}

fn decode(raw: ValueRef<'_>, carrier: ScalarType, work: &Work) -> Result<Value, SqliteError> {
    match (raw, carrier) {
        (ValueRef::Null, _) => Ok(Value::Null),
        (ValueRef::Integer(value), ScalarType::Int64) => Ok(Value::Int64(value)),
        (ValueRef::Real(value), ScalarType::Double) => Ok(Value::Double(Double::from_float(value))),
        (ValueRef::Text(bytes), ScalarType::Text) => Ok(Value::Text(work.text(bytes)?)),
        (ValueRef::Blob(bytes), ScalarType::Binary) => {
            let mut owned = Vec::new();
            work.reserve(&mut owned, bytes.len())?;
            owned.extend_from_slice(bytes);
            Ok(Value::Binary(owned))
        }
        _ => Err(SqliteError::Value),
    }
}

fn selection(name: &str, columns: &[Column], work: &Work) -> Result<String, SqliteError> {
    // SchemaPolicy bounds both counts and identity bytes before this calculation.
    // Each byte can at most double when quoting identifiers; reserve before append.
    let capacity = "SELECT  FROM main.\"\"".len()
        + name.len() * 2
        + columns
            .iter()
            .map(|c| c.name().len() * 2 + "\"\",".len())
            .sum::<usize>();
    work.charge(Resource::Bytes, capacity as u64)?;
    let mut sql = String::new();
    sql.try_reserve_exact(capacity)
        .map_err(|_| SqliteError::Allocation)?;
    sql.push_str("SELECT ");
    for (position, column) in columns.iter().enumerate() {
        if position != 0 {
            sql.push(',');
        }
        quote(column.name(), &mut sql);
    }
    sql.push_str(" FROM main.");
    quote(name, &mut sql);
    Ok(sql)
}

fn quote(name: &str, output: &mut String) {
    output.push('"');
    for character in name.chars() {
        if character == '"' {
            output.push('"');
        }
        output.push(character);
    }
    output.push('"');
}
