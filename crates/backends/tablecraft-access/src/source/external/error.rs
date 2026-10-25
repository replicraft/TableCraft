use super::super::LinkSourceError;
use tablecraft_query::QueryError;
use thiserror::Error;

/// Payload-free setup failures for explicitly supplied native external sources.
#[derive(Debug, Error)]
pub enum ExternalSourceError {
    /// Source lookup, cancellation or resource controls failed.
    #[error("external source setup failed")]
    Query(#[from] QueryError),
    /// Shared native allowance or caller-scoped identities disagree.
    #[error("external native source identity validation failed")]
    Identity(#[from] LinkSourceError),
    /// Exact database tokens must identify one binding.
    #[error("external source database tokens overlap")]
    DuplicateToken,
    /// Each exact token binding requires a distinct host key in this facade.
    #[error("external source binding keys overlap")]
    DuplicateKey,
}
