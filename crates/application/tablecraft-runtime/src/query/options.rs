//! Explicit, bounded allowances captured by one read-query command.
use tablecraft_ace::TableSnapshotLimits;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_query::QueryLimits;

/// Identified resource pool at a runtime boundary. Execution errors that have
/// already crossed shared source/expression contracts retain their original type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryRunPool {
    Native,
    Presentation,
}

/// Validated cumulative resource allowances, rather than resident memory limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryPoolLimits {
    bytes: u64,
    rows: u64,
    steps: u64,
}

impl QueryPoolLimits {
    pub const MAX_BYTES: u64 = 4 * 1024 * 1024 * 1024;
    pub const MAX_ROWS: u64 = 10_000_000;
    pub const MAX_STEPS: u64 = 1_000_000_000;

    /// Bytes and steps must be positive; non-row pools use zero rows.
    pub fn new(bytes: u64, rows: u64, steps: u64) -> Result<Self, QueryRunOptionsError> {
        if bytes == 0
            || bytes > Self::MAX_BYTES
            || rows > Self::MAX_ROWS
            || steps == 0
            || steps > Self::MAX_STEPS
        {
            return Err(QueryRunOptionsError::PoolBounds);
        }

        Ok(Self { bytes, rows, steps })
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    pub const fn rows(self) -> u64 {
        self.rows
    }

    pub const fn steps(self) -> u64 {
        self.steps
    }

    pub(crate) const fn budget(self) -> ResourceBudget {
        ResourceBudget::new(self.bytes, self.rows, self.steps)
    }

    pub(crate) const fn query_limits(self) -> QueryLimits {
        QueryLimits {
            bytes: self.bytes,
            rows: self.rows,
            steps: self.steps,
        }
    }
}

/// Invalid host configuration; no database work has started.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum QueryRunOptionsError {
    #[error("query snapshot row or batch allowance is outside the supported bounds")]
    SnapshotBounds,
    #[error("query resource allowances are outside the supported bounds")]
    PoolBounds,
    #[error("query execution allowances exceed the retrieval service ceilings")]
    QueryBounds,
    #[error("native and query pools require a positive row allowance")]
    RowAllowance,
    #[error("comparison and presentation pools do not admit source rows")]
    NonRowPool,
}

/// One captured read run's independent source, execution, comparison and output
/// allowances. Increasing these never changes format, index or payload limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryRunOptions {
    snapshot: TableSnapshotLimits,
    native: QueryPoolLimits,
    query: QueryPoolLimits,
    comparison: QueryPoolLimits,
    presentation: QueryPoolLimits,
}

impl QueryRunOptions {
    /// Construct snapshot controls without requiring hosts to import the engine.
    pub fn with_limits(
        snapshot_rows: u32,
        batch_rows: u32,
        native: QueryPoolLimits,
        query: QueryPoolLimits,
        comparison: QueryPoolLimits,
        presentation: QueryPoolLimits,
    ) -> Result<Self, QueryRunOptionsError> {
        let snapshot = TableSnapshotLimits::new(snapshot_rows, batch_rows)
            .map_err(|_| QueryRunOptionsError::SnapshotBounds)?;
        Self::new(snapshot, native, query, comparison, presentation)
    }

    pub fn new(
        snapshot: TableSnapshotLimits,
        native: QueryPoolLimits,
        query: QueryPoolLimits,
        comparison: QueryPoolLimits,
        presentation: QueryPoolLimits,
    ) -> Result<Self, QueryRunOptionsError> {
        if native.rows == 0 || query.rows == 0 {
            return Err(QueryRunOptionsError::RowAllowance);
        }
        if comparison.rows != 0 || presentation.rows != 0 {
            return Err(QueryRunOptionsError::NonRowPool);
        }
        if query.bytes > QueryLimits::MAX_BYTES
            || query.rows > QueryLimits::MAX_ROWS
            || query.steps > QueryLimits::MAX_STEPS
        {
            return Err(QueryRunOptionsError::QueryBounds);
        }

        Ok(Self {
            snapshot,
            native,
            query,
            comparison,
            presentation,
        })
    }

    pub const fn snapshot(self) -> TableSnapshotLimits {
        self.snapshot
    }

    pub const fn native(self) -> QueryPoolLimits {
        self.native
    }

    pub const fn query(self) -> QueryPoolLimits {
        self.query
    }

    pub const fn comparison(self) -> QueryPoolLimits {
        self.comparison
    }

    pub const fn presentation(self) -> QueryPoolLimits {
        self.presentation
    }
}

impl Default for QueryRunOptions {
    fn default() -> Self {
        Self {
            snapshot: TableSnapshotLimits::default(),
            native: QueryPoolLimits {
                bytes: 512 * 1024 * 1024,
                rows: 100_000,
                steps: 5_000_000,
            },
            query: QueryPoolLimits {
                bytes: 16 * 1024 * 1024,
                rows: 10_000,
                steps: 1_000_000,
            },
            comparison: QueryPoolLimits {
                bytes: 16 * 1024 * 1024,
                rows: 0,
                steps: 1_000_000,
            },
            presentation: QueryPoolLimits {
                bytes: 8 * 1024 * 1024,
                rows: 0,
                steps: 100_000,
            },
        }
    }
}

pub(crate) fn native_error(error: tablecraft_ace::InspectError) -> crate::Error {
    match error {
        tablecraft_ace::InspectError::Limit(source) => crate::Error::ReadQueryLimit {
            pool: QueryRunPool::Native,
            source,
        },
        other => other.into(),
    }
}

pub(crate) fn presentation_error(error: crate::Error) -> crate::Error {
    match error {
        crate::Error::Limit(source) => crate::Error::ReadQueryLimit {
            pool: QueryRunPool::Presentation,
            source,
        },
        other => other,
    }
}
