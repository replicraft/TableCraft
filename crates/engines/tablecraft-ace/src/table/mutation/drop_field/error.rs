use crate::{
    CreateDatabaseError, CreatedAddColumnError, EditError, FieldHistoryDatabaseOptionsError,
    InspectError,
};
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Payload-free refusal of the separate saved-field DROP working profile.
#[derive(Debug, Error)]
pub enum FieldDropError {
    /// Native inspection, cumulative budget, allocation or cancellation failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The complete original field-history/OFF reconstruction failed.
    #[error(transparent)]
    SourceProof(#[from] FieldHistoryDatabaseOptionsError),
    /// Existing constructive declaration or policy observation failed.
    #[error(transparent)]
    Metadata(#[from] CreatedAddColumnError),
    /// Remaining declarations cannot retain the exact known dictionary/policies.
    #[error(transparent)]
    Declaration(#[from] CreateDatabaseError),
    /// The actual fitted property placement or parent update failed.
    #[error(transparent)]
    Placement(#[from] EditError),
    /// The selected identity, declaration, namespace or dependency is unsupported.
    #[error("saved field DROP source is outside the supported profile")]
    Source,
    /// Output changed a protected byte, identity or surviving policy.
    #[error("saved field DROP preservation check failed")]
    Preservation,
}

impl From<LimitError> for FieldDropError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

impl From<super::super::saved_field::Error> for FieldDropError {
    fn from(error: super::super::saved_field::Error) -> Self {
        match error {
            super::super::saved_field::Error::Inspect(error) => Self::Inspect(error),
            super::super::saved_field::Error::Source => Self::Source,
        }
    }
}

impl From<crate::creation::properties::BodyError> for FieldDropError {
    fn from(error: crate::creation::properties::BodyError) -> Self {
        match error {
            crate::creation::properties::BodyError::Declaration(error) => Self::Declaration(error),
            crate::creation::properties::BodyError::Inspect(error) => Self::Inspect(error),
        }
    }
}
