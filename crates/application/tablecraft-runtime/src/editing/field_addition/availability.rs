//! Revision/table-bound source facts, never a named target or write capability.
use crate::{Error, Response, document::Document, editing::platform};
use std::sync::Arc;
use tablecraft_builders::table::TypedFieldAdditionAvailability;

#[derive(Debug, PartialEq, Eq)]
pub struct FieldAdditionAvailability {
    pub document_revision: u64,
    pub table: String,
    pub number_available: bool,
    pub short_text_available: bool,
    pub typed_declarations: Arc<[TypedFieldAdditionAvailability]>,
}

impl Document {
    pub(crate) fn inspect_field_addition_availability(
        &mut self,
        document_revision: u64,
        table: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::field_addition::availability::inspect(self, table, cancelled)
    }
}
