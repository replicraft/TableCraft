use tablecraft_schema::FieldConstraintViolation;
use thiserror::Error;

/// Payload-free violations found by an explicit native constraint audit.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ConstraintViolation {
    /// A supported field validation expression returned FALSE.
    #[error("field validation rule rejected the record")]
    FieldRule,
    /// A supported table validation expression returned FALSE.
    #[error("table validation rule rejected the record")]
    TableRule,
    /// Pure scalar evaluation failed; names, expressions and values are omitted.
    #[error("validation expression failed: {0}")]
    Evaluation(tablecraft_expressions::EvalError),
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
