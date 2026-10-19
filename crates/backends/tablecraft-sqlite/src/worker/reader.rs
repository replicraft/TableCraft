use super::{AsyncError, AsyncOptions, cancellation::CancelOnDrop};
use crate::{SqliteSource, source::finish_read, work::Work};
use rusqlite::OpenFlags;
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tablecraft_core::values::Value;
use tablecraft_query::{ExecutionOptions, QueryResult, prepare};
use tablecraft_sql::query::Query;
use tokio::sync::Semaphore;

const MAX_PARAMETERS: usize = 256;

/// Worker-owned read-only SQLite connection. Each admitted query owns its entire
/// snapshot and cleanup on the worker. One request is allowed per reader; excess
/// requests fail immediately instead of entering an unbounded worker queue.
/// This handle intentionally does not implement Clone or expose the connection.
///
/// ```no_run
/// use std::{path::Path, sync::{Arc, atomic::AtomicBool}};
/// use tablecraft_core::limits::ResourceBudget;
/// use tablecraft_sqlite::{AsyncOptions, AsyncReader};
///
/// async fn read(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
///     let reader = AsyncReader::open(path).await?;
///     let parsed = tablecraft_sql::parse_query(
///         "SELECT TOP 5 ID FROM Items ORDER BY ID", Default::default(),
///     )?;
///     let options = AsyncOptions::new(
///         ResourceBudget::new(16 * 1024 * 1024, 16_384, 1_000_000),
///         Arc::new(AtomicBool::new(false)),
///     );
///     let result = reader.query(parsed.query().clone(), vec![], options).await?;
///     assert!(result.rows().len() <= 5);
///     reader.close().await?;
///     Ok(())
/// }
/// ```
pub struct AsyncReader {
    connection: tokio_rusqlite::Connection,
    gate: Arc<Semaphore>,
    healthy: Arc<AtomicBool>,
}

impl AsyncReader {
    /// Open an existing literal filesystem path on a dedicated worker thread.
    /// Opening never creates a file. Filesystem stalls are not hard-deadline
    /// bounded; query budgets/cancellation apply to subsequent query operations.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, AsyncError> {
        let connection = tokio_rusqlite::Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .await
        .map_err(|error| AsyncError::Storage(error.into()))?;
        Ok(Self {
            connection,
            gate: Arc::new(Semaphore::new(1)),
            healthy: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Evaluate an owned Access query on the worker using TableCraft semantics.
    /// Parameters and collation stay inside the admitted request. Dropping this
    /// future sets its cancellation flag; the admission permit stays with the
    /// actual worker job through rollback, even after its caller stops awaiting.
    /// A dropped future does not prove the job or filesystem I/O already stopped.
    pub async fn query(
        &self,
        query: Query,
        parameters: Vec<(String, Value)>,
        options: AsyncOptions,
    ) -> Result<QueryResult, AsyncError> {
        if !self.healthy.load(Ordering::Acquire) {
            return Err(AsyncError::Worker);
        }
        let permit = self
            .gate
            .clone()
            .try_acquire_owned()
            .map_err(|_| AsyncError::Busy)?;
        if !self.healthy.load(Ordering::Acquire) {
            return Err(AsyncError::Worker);
        }
        let mut cancellation = CancelOnDrop::new(options.cancelled.clone());
        let healthy = self.healthy.clone();

        let result =
            self.connection
                .call_raw(move |connection| {
                    let _permit = permit;
                    let _span = tracing::debug_span!("sqlite.worker.query").entered();
                    let work = Work::new(options.storage_budget, options.cancelled.clone());
                    let result = (|| {
                        let source = SqliteSource::initialize(&*connection, work.clone())?;
                        let plan = prepare(&query, &source)?;
                        if parameters.len() > MAX_PARAMETERS {
                            return Err(AsyncError::Query(
                                tablecraft_query::QueryError::StructureLimit,
                            ));
                        }
                        // Move owned payloads rather than cloning them before the query
                        // evaluator has applied its own value/materialization limits.
                        let mut names = Vec::new();
                        let mut values = Vec::new();
                        let mut bindings = Vec::new();
                        work.reserve(&mut names, parameters.len())?;
                        work.reserve(&mut values, parameters.len())?;
                        work.reserve(&mut bindings, parameters.len())?;
                        for (name, value) in parameters {
                            names.push(name);
                            values.push(value);
                        }
                        for (name, value) in names.iter().zip(values) {
                            bindings.push((name.as_str(), value));
                        }
                        let cancelled = || options.cancelled.load(Ordering::Relaxed);
                        plan.execute(
                            &source,
                            &bindings,
                            ExecutionOptions {
                                limits: options.query_limits,
                                like_mode: options.like_mode,
                                collation: options.collation.as_deref().map(|c| {
                                    c as &dyn tablecraft_expressions::comparison::Collation
                                }),
                                cancelled: Some(&cancelled),
                            },
                        )
                        .map_err(AsyncError::from)
                    })();

                    // Cleanup is part of the admitted job. Disable the exhausted callback
                    // before rollback; refuse reuse if teardown cannot be confirmed.
                    if let Err(error) = finish_read(connection) {
                        healthy.store(false, Ordering::Release);
                        return Err(AsyncError::Storage(error));
                    }
                    result
                })
                .await
                .map_err(|_| {
                    self.healthy.store(false, Ordering::Release);
                    AsyncError::Worker
                })?;
        cancellation.disarm();
        result
    }

    /// Drain already admitted work and report worker close failure. Callers
    /// should cancel a pending query before closing when they want it to stop.
    pub async fn close(self) -> Result<(), AsyncError> {
        self.connection
            .close()
            .await
            .map_err(|_| AsyncError::Worker)
    }
}

#[cfg(test)]
mod tests;
