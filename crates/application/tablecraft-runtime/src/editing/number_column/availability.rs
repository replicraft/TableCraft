//! Revision/selector-bound presentation facts from the actual source proof.
use crate::{Error, Response, document::Document, editing::platform};

/// Positive source-profile inspection for opening a local Number naming editor.
/// This is neither a reviewed target declaration nor a write/recovery capability.
/// Fresh named review and consuming save repeat native source qualification.
#[derive(Debug, PartialEq, Eq)]
pub struct NumberColumnAvailability {
    pub document_revision: u64,
    /// Exact requested table selector, bound to the same opened revision.
    pub table: String,
    pub available: bool,
}

impl Document {
    pub(crate) fn inspect_number_column_availability(
        &mut self,
        document_revision: u64,
        table: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::number_column::availability::inspect(self, table, cancelled)
    }
}
