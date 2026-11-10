//! Field-addition failures are data even when native creation is disabled.
#[cfg(feature = "ace16-create")]
use super::current_schema;
use crate::{EditError, InspectError};
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Payload-free failure of current native column addition.
#[derive(Debug, Error)]
pub enum CurrentFieldAdditionError {
    /// Native parsing, I/O, cancellation, allocation or cumulative limits failed.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// Current programs, records, constraints or owned placement were refused.
    #[error(transparent)]
    Native(#[from] EditError),
    /// Current native roles, declarations or dependencies are unsupported.
    #[error("native table is outside the current field-addition source profile")]
    Source,
    /// The proposed name is not a valid qualified ASCII identifier.
    #[error("current field addition requires a valid ASCII field name")]
    InvalidName,
    /// An existing native field has the proposed name under ASCII folding.
    #[error("field addition conflicts with an existing field")]
    DuplicateName,
    /// The descriptor, name or complete new property body cannot fit owned space.
    #[error("current field addition exceeds existing owned capacity")]
    Capacity,
    /// The candidate changed an unrelated byte or decoded native fact.
    #[error("current field-addition preservation check failed")]
    Preservation,
}

impl From<LimitError> for CurrentFieldAdditionError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

#[cfg(feature = "ace16-create")]
impl From<current_schema::Error> for CurrentFieldAdditionError {
    fn from(error: current_schema::Error) -> Self {
        match error {
            current_schema::Error::Inspect(error) => Self::Inspect(error),
            current_schema::Error::Native(error) => Self::Native(error),
            current_schema::Error::Source => Self::Source,
        }
    }
}
