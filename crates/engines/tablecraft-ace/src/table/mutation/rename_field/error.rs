use crate::{EditError, FieldHistoryDatabaseOptionsError, InspectError};
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Payload-free refusal of the separately qualified field-rename families.
#[derive(Debug, Error)]
pub enum FieldRenameError {
    /// Native parsing, allocation, cumulative resource or cancellation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Complete original field-history/OFF reconstruction or replay failed.
    #[error(transparent)]
    SourceProof(#[from] FieldHistoryDatabaseOptionsError),
    /// Actual native records, constraints, ownership or row replay failed.
    #[error(transparent)]
    RowSource(#[from] EditError),
    /// The proposed name violates native identifier syntax or its size ceiling.
    #[error("field rename requires a valid native identifier")]
    InvalidName,
    /// Only equal UTF-16 width ASCII name changes are currently qualified.
    #[error("field rename currently requires equal-width ASCII names")]
    NameShape,
    /// Populated field names must use the qualified ASCII identifier namespace.
    #[error("populated field rename requires ASCII names")]
    PopulatedNameShape,
    /// Current metadata names must use the qualified ASCII namespace.
    #[error("current field rename requires ASCII names")]
    CurrentNameShape,
    /// The fitted profile is separate from the existing same-width/no-op route.
    #[error("fitted field rename requires unequal-width ASCII names")]
    FittedNameShape,
    /// The name cannot fit the original definition and property page capacity.
    #[error("fitted field rename exceeds the existing name capacity")]
    FittedCapacity,
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

impl From<crate::table::mutation::saved_field::Error> for FieldRenameError {
    fn from(error: crate::table::mutation::saved_field::Error) -> Self {
        match error {
            crate::table::mutation::saved_field::Error::Inspect(error) => Self::Inspect(error),
            crate::table::mutation::saved_field::Error::Source => Self::Source,
        }
    }
}
