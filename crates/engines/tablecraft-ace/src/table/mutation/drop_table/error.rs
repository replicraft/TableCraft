use crate::{InspectError, TableRenameError};
use tablecraft_core::limits::LimitError;
use thiserror::Error;

/// Payload-free refusal of an unqualified logical native table deletion.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TableDropError {
    /// Inspection, cancellation, resource or fallible allocation failure.
    #[error(transparent)]
    Inspect(#[from] InspectError),
    /// The requested ordinary local table is absent from the captured catalog.
    #[error("native table deletion target is absent")]
    MissingTable,
    /// The native format, catalog, indexes or physical source is unsupported.
    #[error("native table deletion source is unsupported")]
    Source,
    /// Name tracking, permissions, relationships or application dependencies refuse.
    #[error("native table deletion requires disabled name tracking and no dependencies")]
    Dependency,
    /// The candidate does not preserve the exact logical deletion scope.
    #[error("native table deletion preservation proof failed")]
    Preservation,
}

impl From<TableRenameError> for TableDropError {
    fn from(error: TableRenameError) -> Self {
        match error {
            TableRenameError::Inspect(error) => Self::Inspect(error),
            TableRenameError::Dependency => Self::Dependency,
            TableRenameError::Preservation => Self::Preservation,
            TableRenameError::InvalidName
            | TableRenameError::DuplicateName
            | TableRenameError::Source
            | TableRenameError::Capacity => Self::Source,
        }
    }
}

impl From<LimitError> for TableDropError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}
