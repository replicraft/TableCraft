mod scan;
mod schema;

use crate::{SqliteError, limits::*, work::Work};
use rusqlite::{Connection, OpenFlags, config::DbConfig, limits::Limit};
use std::{
    borrow::Borrow,
    cell::OnceCell,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_query::{QueryError, Source};
use tablecraft_schema::{Column, SchemaName};

struct Entry {
    name: SchemaName,
    columns: OnceCell<Vec<Column>>,
}

/// One existing SQLite file, opened read-only with a pinned read transaction.
/// Tables are inventoried eagerly; scalar schemas and records are read on demand.
/// Query expressions execute in TableCraft, not SQLite. Repeated scans consume
/// the same cumulative budget. Dropping the source releases its snapshot/locks.
/// Cancellation is checked at adapter boundaries and through a SQLite VM hook;
/// filesystem stalls are not hard-deadline bounded by that hook.
pub struct SqliteSource<C = Connection> {
    connection: C,
    work: Work,
    entries: Vec<Entry>,
}

impl SqliteSource {
    /// Open an existing literal filesystem path; never create a database or enable
    /// URI options/extensions. No lock retries. Set the shared atomic flag to
    /// cancel from another thread. No partial source is returned on failure.
    pub fn open(
        path: impl AsRef<Path>,
        budget: ResourceBudget,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self, SqliteError> {
        let work = Work::new(budget, cancelled);
        work.check()?;
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::initialize(connection, work)
    }

    /// Release the read transaction and report close failures explicitly.
    /// Drop also releases resources, without reporting teardown errors.
    pub fn close(self) -> Result<(), SqliteError> {
        // Cancellation/budget exhaustion must not prevent resource release.
        finish_read(&self.connection)?;
        self.connection.close().map_err(|(_, error)| error.into())
    }
}

pub(crate) fn finish_read(connection: &Connection) -> Result<(), SqliteError> {
    connection.progress_handler(0, None::<fn() -> bool>)?;
    if !connection.is_autocommit() {
        connection.execute_batch("ROLLBACK")?;
    }
    Ok(())
}

impl<C: Borrow<Connection>> SqliteSource<C> {
    pub(crate) fn initialize(connection: C, work: Work) -> Result<Self, SqliteError> {
        let _span = tracing::debug_span!("sqlite.snapshot.open").entered();
        work.check()?;
        let db = connection.borrow();
        db.busy_timeout(Duration::ZERO)?;
        db.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;
        db.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
        db.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, false)?;
        db.set_db_config(DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false)?;
        for (limit, value) in [
            (Limit::SQLITE_LIMIT_LENGTH, MAX_VALUE_BYTES as i32),
            (Limit::SQLITE_LIMIT_SQL_LENGTH, MAX_SQL_BYTES as i32),
            (Limit::SQLITE_LIMIT_COLUMN, SCHEMA_POLICY.max_columns as i32),
            (Limit::SQLITE_LIMIT_EXPR_DEPTH, MAX_EXPRESSION_DEPTH),
            (Limit::SQLITE_LIMIT_VDBE_OP, MAX_VM_PROGRAM_OPS),
            (Limit::SQLITE_LIMIT_ATTACHED, 0),
            (Limit::SQLITE_LIMIT_WORKER_THREADS, 0),
        ] {
            db.set_limit(limit, value)?;
        }
        work.install(db)?;
        work.sql(db.execute_batch("PRAGMA query_only=ON; BEGIN DEFERRED"))?;
        // Pin the main database snapshot with an actual b-tree read; a
        // metadata-only PRAGMA must not be relied upon to start the snapshot.
        work.sql(
            db.query_row("SELECT count(*) FROM main.sqlite_schema", [], |row| {
                row.get::<_, i64>(0)
            }),
        )?;
        let entries = schema::inventory(db, &work)?;
        work.check()?;
        Ok(Self {
            connection,
            work,
            entries,
        })
    }

    fn entry(&self, name: &str) -> Result<&Entry, QueryError> {
        self.work.check()?;
        self.entries
            .iter()
            .find(|entry| SCHEMA_POLICY.names.equals(entry.name.as_str(), name))
            .ok_or(QueryError::UnknownName)
    }
}

impl<C: Borrow<Connection>> Source for SqliteSource<C> {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        let entry = self.entry(table)?;
        if entry.columns.get().is_none() {
            let columns =
                schema::columns(self.connection.borrow(), entry.name.as_str(), &self.work)?;
            entry.columns.set(columns).map_err(|_| QueryError::Source)?;
        }
        entry
            .columns
            .get()
            .map(Vec::as_slice)
            .ok_or(QueryError::Source)
    }

    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scan_table(table, visitor)
    }
}

#[cfg(test)]
mod tests;
