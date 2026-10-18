use thiserror::Error;

/// An explicit complex storage view has inconsistent parent/child references.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ComplexReferenceViolation {
    /// More than one parent row has the same assigned key in this column.
    #[error("ambiguous native complex parent reference")]
    DuplicateParent,
    /// A child identity is repeated in its resolved flat table.
    #[error("duplicate native complex child identity")]
    DuplicateChild,
    /// A child foreign key has no assigned parent key in this column.
    #[error("native complex child has no parent binding")]
    MissingParent,
    /// A resolved child foreign key or identity is NULL.
    #[error("NULL native complex child reference")]
    NullChild,
}
