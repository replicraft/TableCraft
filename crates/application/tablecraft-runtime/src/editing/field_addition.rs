//! Closed field declarations reviewed through one revision-bound native owner.
mod availability;
mod origin;
mod outcome;
use super::{AddedColumnReopen, platform};
use crate::{Error, Response, document::Document};
pub use availability::FieldAdditionAvailability;
pub use origin::FieldAdditionOrigin;
pub use outcome::{FieldAdditionOutcomeError, FieldAdditionReopenFacts};
use std::{path::Path, sync::Arc};
use tablecraft_builders::table::FieldAdditionDeclaration;

/// Exact declaration captured by a native original-intent plan.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldAdditionReview {
    pub id: u64,
    pub document_revision: u64,
    pub table: String,
    pub column: String,
    pub origin: FieldAdditionOrigin,
}

/// Consuming field-addition authority; presentation facts cannot construct it.
pub struct ReviewedFieldAddition {
    pub(super) state: Box<platform::field_addition::State>,
}

impl ReviewedFieldAddition {
    pub fn source_path(&self) -> &Path {
        self.state.reopen.source_path()
    }

    pub fn reopen_witness(&self) -> Arc<AddedColumnReopen> {
        Arc::clone(&self.state.reopen)
    }

    /// Derived from the private checked native plan when this owner was issued.
    pub fn captured_origin(&self) -> FieldAdditionOrigin {
        self.state.origin
    }

    /// Private native review identity, distinct from host request sequencing.
    pub fn id(&self) -> u64 {
        self.state.id
    }
}

impl Document {
    pub(crate) fn review_add_field(
        &mut self,
        document_revision: u64,
        table: &str,
        column: &str,
        declaration: FieldAdditionDeclaration,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        if document_revision != self.revision {
            return Err(Error::StaleDocument);
        }
        platform::field_addition::review(self, table, column, declaration, cancelled)
    }
}
