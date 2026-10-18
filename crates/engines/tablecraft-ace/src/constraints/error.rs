use tablecraft_schema::FieldConstraintViolation;
use thiserror::Error;

/// Payload-free violations found by an explicit native constraint audit.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ConstraintViolation {
    /// A scalar field violates its qualified Required/AllowZeroLength rule.
    #[error(transparent)]
    Field(#[from] FieldConstraintViolation),
    /// A dependent non-NULL key has no referenced parent.
    #[error("foreign key has no referenced record")]
    MissingParent,
    /// A referenced key is duplicated despite its qualified unique declaration.
    #[error("referenced key is not unique")]
    DuplicateParent,
    /// A one-to-one declaration has repeated non-NULL dependent keys.
    #[error("one-to-one relationship has duplicate dependent keys")]
    DuplicateChild,
}
