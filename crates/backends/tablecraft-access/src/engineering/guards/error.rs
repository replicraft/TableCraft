use crate::NativeEditError;
use tablecraft_ace::{BeforeEventGuardError, InspectError};
use tablecraft_query::{DeleteError, InsertError, QueryError, UpdateError};
use thiserror::Error;

/// Source-bound guard finish failures, without SQL/XML/row payload diagnostics.
#[derive(Debug, Error)]
pub enum GuardBackendError {
    /// Existing snapshot/schema/expected-value qualification failed.
    #[error(transparent)]
    Native(#[from] NativeEditError),
    /// Native guard preparation refused or failed, with no completed report.
    #[error(transparent)]
    Guard(#[from] BeforeEventGuardError),
    /// Shared resources or cancellation while retaining report facts.
    #[error(transparent)]
    Inspect(#[from] InspectError),
}

/// Existing neutral statement planning or explicit guard finish failure.
#[derive(Debug, Error)]
pub enum GuardSqlError {
    /// Closed statement shape or neutral binding failed.
    #[error(transparent)]
    Query(QueryError),
    /// Captured source or guard result preparation failed.
    #[error(transparent)]
    Backend(GuardBackendError),
    /// UPDATE predicates/assignments or guard finish failed.
    #[error(transparent)]
    Update(UpdateError<GuardBackendError>),
    /// DELETE predicates or guard finish failed.
    #[error(transparent)]
    Delete(DeleteError<GuardBackendError>),
    /// Complete VALUES binding/evaluation or guard finish failed.
    #[error(transparent)]
    Insert(InsertError<GuardBackendError>),
}
