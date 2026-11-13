//! Native QueryDef creation candidate; no journal or application permission.
mod definition;
pub(crate) mod effect;
#[cfg(test)]
mod tests;

use crate::{EditError, FileFormat, InspectError, NativeImage, header::check};
pub use definition::QueryCreationDefinition;
use tablecraft_core::limits::{LimitError, ResourceBudget};
use thiserror::Error;

/// Refusal from native constant-SELECT object construction and fresh proof.
#[derive(Debug, Error)]
pub enum QueryCreationError {
    /// Native capture, cancellation, allocation or caller allowance failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Destination, alias or requested constant declaration is invalid.
    #[error("native query creation declaration is unsupported")]
    Definition,
    /// Source schemas, ownership, programs or available placement are unsupported.
    #[error("native query creation source or placement is unsupported")]
    Source,
    /// Fresh readback or complete-image preservation did not match the request.
    #[error("native query creation preservation proof failed")]
    Preservation,
}

impl From<LimitError> for QueryCreationError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

impl From<EditError> for QueryCreationError {
    fn from(error: EditError) -> Self {
        match error {
            EditError::Inspect(error) => Self::Inspect(error),
            EditError::Preservation => Self::Preservation,
            _ => Self::Source,
        }
    }
}

/// Source-bound new QueryDef bytes with independent full-image proof.
///
/// This candidate cannot prepare a journal, save, recover or publish a query.
/// It retains the original image and caller-captured declaration; no mutable
/// image, arbitrary record constructor or candidate-promotion API is exposed.
pub struct NativeQueryCreationCandidate {
    before: NativeImage,
    after: NativeImage,
    definition: QueryCreationDefinition,
    native_id: i32,
}

impl NativeImage {
    /// Construct one new constant SELECT without changing an existing QueryDef.
    ///
    /// The first source profile requires actual qualified native catalog and
    /// MSysQueries storage with complete append ownership. Existing fragments,
    /// user records and protected metadata remain unchanged. The declaration
    /// supplies exact creation/update DATE bits; no clock or SQL is executed.
    ///
    /// # Errors
    /// Refuses incompatible source policy/geometry, collisions, unsupported
    /// metadata, cancellation, caller limits and failed fresh preservation proof.
    pub fn prepare_query_creation(
        self,
        definition: QueryCreationDefinition,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<NativeQueryCreationCandidate, QueryCreationError> {
        let _span = tracing::debug_span!("native.query_creation.prepare").entered();
        check(budget, cancelled)?;
        self.require_retained_capture()?;
        let header = self.catalog().header();
        if header.format() != FileFormat::Ace2016
            || header.subversion() != 0
            || header.encoded_pages()
        {
            return Err(QueryCreationError::Source);
        }

        let built =
            crate::table::mutation::query_creation::build(&self, &definition, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(NativeQueryCreationCandidate {
            before: self,
            after: built.after,
            definition,
            native_id: built.native_id,
        })
    }
}

impl NativeQueryCreationCandidate {
    /// Complete immutable original image admitted by this construction.
    pub fn before_bytes(&self) -> &[u8] {
        self.before.as_bytes()
    }

    /// Complete independently checked image; this is not a save capability.
    pub fn candidate_bytes(&self) -> &[u8] {
        self.after.as_bytes()
    }

    /// Original captured constant, names and timestamp with no copies.
    pub fn definition(&self) -> &QueryCreationDefinition {
        &self.definition
    }

    /// Allocated signed native query identity, separate from page/ROW addresses.
    pub const fn native_query_id(&self) -> i32 {
        self.native_id
    }
}
