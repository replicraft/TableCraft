use super::Entry;
use crate::{
    SqliteError,
    limits::{MAX_TABLES, SCHEMA_POLICY},
    work::Work,
};
use rusqlite::{Connection, Row, types::ValueRef};
use std::cell::OnceCell;
use tablecraft_schema::{Column, ScalarType, SchemaName};

pub(super) fn inventory(connection: &Connection, work: &Work) -> Result<Vec<Entry>, SqliteError> {
    // table_list distinguishes ordinary tables from virtual tables, their shadow
    // tables and views. No application SQL or virtual table module is invoked.
    let mut statement = work.sql(connection.prepare(
        "SELECT name FROM pragma_table_list WHERE schema='main' AND type='table' \
         AND name NOT GLOB 'sqlite_*' ORDER BY name",
    ))?;
    let mut rows = work.sql(statement.query([]))?;
    let mut entries = Vec::new();
    while let Some(row) = work.sql(rows.next())? {
        work.check()?;
        if entries.len() == MAX_TABLES {
            return Err(SqliteError::StructureLimit);
        }
        let name = SchemaName::new(text(row, 0, work)?)?;
        SCHEMA_POLICY.validate_name(&name)?;
        if entries.iter().any(|entry: &Entry| {
            SCHEMA_POLICY
                .names
                .equals(entry.name.as_str(), name.as_str())
        }) {
            return Err(tablecraft_schema::SchemaError::Duplicate.into());
        }
        work.reserve(&mut entries, 1)?;
        entries.push(Entry {
            name,
            columns: OnceCell::new(),
        });
    }
    Ok(entries)
}

pub(super) fn columns(
    connection: &Connection,
    name: &str,
    work: &Work,
) -> Result<Vec<Column>, SqliteError> {
    let mut statement = work.sql(connection.prepare(
        "SELECT name,type,\"notnull\",hidden FROM pragma_table_xinfo(?1,'main') ORDER BY cid",
    ))?;
    let mut rows = work.sql(statement.query([name]))?;
    let mut columns = Vec::new();
    while let Some(row) = work.sql(rows.next())? {
        work.check()?;
        if columns.len() == SCHEMA_POLICY.max_columns {
            return Err(SqliteError::StructureLimit);
        }
        if work.sql(row.get::<_, i32>(3))? != 0 {
            return Err(SqliteError::Unsupported);
        }
        let name = text(row, 0, work)?;
        let mut declared = text(row, 1, work)?;
        declared.make_ascii_uppercase();
        let scalar = carrier(&declared)?;
        // SQLite's ordinary primary keys may admit NULL. Do not infer NOT NULL
        // from the primary-key ordinal or force Access primary-key semantics.
        let nullable = work.sql(row.get::<_, i32>(2))? == 0;
        work.reserve(&mut columns, 1)?;
        columns.push(Column::new(name, scalar, nullable)?);
    }
    SCHEMA_POLICY.validate_columns(&columns)?;
    Ok(columns)
}

fn carrier(declared: &str) -> Result<ScalarType, SqliteError> {
    // SQLite affinity precedence, restricted to carriers we can represent without
    // guessing a decimal/date/boolean encoding or inspecting arbitrary data.
    if declared.contains("INT") {
        Ok(ScalarType::Int64)
    } else if ["CHAR", "CLOB", "TEXT"]
        .iter()
        .any(|part| declared.contains(part))
    {
        Ok(ScalarType::Text)
    } else if declared.contains("BLOB") {
        Ok(ScalarType::Binary)
    } else if ["REAL", "FLOA", "DOUB"]
        .iter()
        .any(|part| declared.contains(part))
    {
        Ok(ScalarType::Double)
    } else {
        Err(SqliteError::Unsupported)
    }
}

fn text(row: &Row<'_>, index: usize, work: &Work) -> Result<String, SqliteError> {
    match work.sql(row.get_ref(index))? {
        ValueRef::Text(bytes) => work.text(bytes),
        _ => Err(SqliteError::Value),
    }
}
