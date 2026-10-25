use tablecraft_ace::InspectError;
use tablecraft_query::QueryError;
use thiserror::Error;

/// Explicit linked-source setup failure. Diagnostics contain no stored names,
/// connection strings, paths or records. A failure publishes no facade.
#[derive(Debug, Error)]
pub enum LinkSourceError {
    /// A native metadata read or its cumulative resource controls failed.
    #[error("linked source metadata inspection failed")]
    Native(#[from] InspectError),
    /// Native scalar schema discovery failed.
    #[error("linked source schema validation failed")]
    Query(#[from] QueryError),
    /// All sources must borrow exactly the same native allowance.
    #[error("linked sources require one shared native allowance")]
    SharedBudgetRequired,
    /// Distinct supplied sources require distinct caller-issued scopes.
    #[error("linked source scopes overlap")]
    ScopeCollision,
    /// A supplied mapping does not name a root link.
    #[error("linked source mapping is extraneous")]
    ExtraMapping,
    /// Two mappings name the same root alias.
    #[error("linked source mappings overlap")]
    DuplicateMapping,
    /// Exact stored identifiers or pinned native identities disagree.
    #[error("linked source mapping does not match stored metadata")]
    IdentityMismatch,
    /// Provider-specific declarations, chains and named targets are unsupported.
    #[error("linked source target is outside the local table profile")]
    UnsupportedTarget,
}
