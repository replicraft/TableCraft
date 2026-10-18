use thiserror::Error;
/// Catalog rejection without identifiers, names or object contents.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    /// Repeated semantic object IDs.
    #[error("duplicate catalog identity")]
    DuplicateIdentity,
    /// An object/parent identity belongs to another catalog scope.
    #[error("catalog identity scope mismatch")]
    Scope,
    /// Name lookup matched more than one object.
    #[error("ambiguous catalog object name")]
    AmbiguousName,
    /// Caller-selected object-count ceiling exceeded.
    #[error("catalog object limit exceeded")]
    Limit,
    /// Fallible index allocation failed.
    #[error("catalog allocation failed")]
    Allocation,
}
