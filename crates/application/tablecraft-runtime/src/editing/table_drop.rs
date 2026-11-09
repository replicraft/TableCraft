//! Closed-table DROP uses metadata inspection, consuming review and a complete-image witness.
mod outcome;
mod review;

pub use outcome::{TableDropOutcomeError, TableDropReopenFacts};
pub use review::{ReviewedTableDrop, TableDropReview, TableDroppedReopen};

use crate::{Error, Response, document::Document, editing::platform};

/// Read-only source facts for one exact current table; never write authority.
#[derive(Debug, PartialEq, Eq)]
pub struct TableDropSourceInspection {
    /// Current document revision whose bytes were inspected.
    pub document_revision: u64,
    /// Exact stored table name, not a caption.
    pub table: String,
    /// Native catalog identity, separate from caller-scoped UI identity.
    pub native_id: i32,
    /// Stored native row count, not an affected-record count.
    pub declared_rows: u32,
    /// Complete qualification for the implemented empty-table source profile.
    pub source_eligible: bool,
}

impl Document {
    pub(crate) fn inspect_table_drop_source(
        &mut self,
        revision: u64,
        table: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::table_drop::inspect(self, table, cancelled)
    }
}
