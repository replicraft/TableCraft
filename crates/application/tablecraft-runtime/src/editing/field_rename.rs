//! Exact field-source facts, consuming rename reviews and checked reopen witnesses.
mod outcome;
mod review;
pub use outcome::{FieldRenameOutcomeError, FieldRenameReopenFacts};
pub use review::{FieldRenameReview, FieldRenamedReopen, ReviewedFieldRename};

use crate::{
    Error, Response,
    controls::{SCOPE, budget, check},
    display::Presentation,
    document::Document,
    editing::platform,
};
use tablecraft_ace::inspect_table_schema;
use tablecraft_builders::table::field_rename::FieldRenameNameProfile;

/// Read-only qualification of one exact native field on one document revision.
/// This is not a consuming review, save capability or source proof token.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldRenameSourceInspection {
    /// Exact current document revision.
    pub document_revision: u64,
    /// Actual native table spelling, never a caption.
    pub table: String,
    /// Actual native field spelling, never a displayed caption or ordinal.
    pub column: String,
    /// Logical native column number from the current schema.
    pub native_number: usize,
    /// Positive complete source/profile qualification on this read host.
    pub source_eligible: bool,
    /// Explicit observed name policy; original width does not imply growth room.
    /// Every replacement still requires fresh consuming native qualification.
    pub name_profile: Option<FieldRenameNameProfile>,
}

impl Document {
    pub(crate) fn inspect_field_rename_source(
        &mut self,
        revision: u64,
        table: &str,
        column: &str,
        native_number: usize,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let _span = tracing::debug_span!("application.inspect_field_rename_source").entered();
        if revision != self.revision {
            return Err(Error::StaleDocument);
        }
        check(cancelled)?;
        let mut work = budget();
        let schema = inspect_table_schema(&mut self.source, SCOPE, table, &mut work, cancelled)?;
        let field = schema
            .columns()
            .iter()
            .find(|field| field.number() == native_number)
            .ok_or(Error::StaleFieldRenameTarget)?;
        if schema.name() != table || field.name() != column {
            return Err(Error::StaleFieldRenameTarget);
        }
        let name_profile = platform::field_rename::source_profile(
            self,
            schema.name(),
            field.name(),
            native_number,
            &mut work,
            cancelled,
        )?;
        let mut output = Presentation::new(cancelled);
        let inspection = FieldRenameSourceInspection {
            document_revision: revision,
            table: output.text(schema.name())?,
            column: output.text(field.name())?,
            native_number,
            source_eligible: name_profile.is_some(),
            name_profile,
        };
        check(cancelled)?;
        Ok(Response::FieldRenameSourceInspected { inspection })
    }
}
