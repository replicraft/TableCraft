use crate::{FieldHistoryDatabaseOptionsError, InspectError};
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Payload-free refusal of this separate same-width field-rename candidate.
#[derive(Debug, Error)]
pub enum FieldRenameError {
    /// Native parsing, allocation, cumulative resource or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete original field-history/OFF reconstruction or replay failed.
    #[error(transparent)]
    SourceProof(#[from] FieldHistoryDatabaseOptionsError),
    /// The proposed name violates native identifier syntax or its size ceiling.
    #[error("field rename requires a valid native identifier")]
    InvalidName,
    /// Only equal UTF-16 width ASCII name changes are currently qualified.
    #[error("field rename currently requires equal-width ASCII names")]
    NameShape,
    /// Another field already has the proposed name under ASCII folding.
    #[error("field rename conflicts with an existing field")]
    DuplicateName,
    /// The requested field, declaration, carrier or dependency is unqualified.
    #[error("native field rename target is outside the supported source profile")]
    Source,
    /// A candidate changed an unrelated byte, identity or decoded property.
    #[error("native field rename preservation check failed")]
    Preservation,
}

impl From<LimitError> for FieldRenameError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}
