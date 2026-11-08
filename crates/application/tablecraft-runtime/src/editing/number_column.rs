//! Explicit GUI Number semantics: nullable LONG, Required false, typed zero.
use super::{AddedColumnReopen, platform};
use crate::{Error, Response, document::Document};
use std::{path::Path, sync::Arc};

/// Consuming capability issued only for the explicit persisted Number declaration.
/// The older nullable Int32/no-default capability cannot substitute for this type.
pub struct ReviewedNumberColumn {
    pub(super) state: Box<platform::number_column::State>,
}

impl ReviewedNumberColumn {
    /// Original source path, without an open write lease during review.
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    /// Exact full expected image for guarded read-only reopening after receipt.
    pub fn reopen_witness(&self) -> Arc<AddedColumnReopen> {
        Arc::clone(&self.state.reopen)
    }
}

impl Document {
    pub(crate) fn review_add_number_column(
        &mut self,
        document_revision: u64,
        table: &str,
        column: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::number_column::review(self, table, column, cancelled)
    }
}
