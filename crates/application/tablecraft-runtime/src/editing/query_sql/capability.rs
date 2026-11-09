//! Full-SQL capability is distinct from the low-level singular literal service.
use crate::editing::{QueryExpressionReopen, platform};
use std::{path::Path, sync::Arc};

/// Only successful same-image full-SQL/native review issues this consuming token.
pub struct ReviewedQuerySql {
    pub(in crate::editing) state: Box<platform::query_sql::State>,
}

impl ReviewedQuerySql {
    /// Exact source selected by the document; stored query text selects no path.
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    /// Shared query-only full-image witness; it cannot save or execute SQL.
    pub fn reopen_witness(&self) -> Arc<QueryExpressionReopen> {
        Arc::clone(&self.state.reopen)
    }
}
