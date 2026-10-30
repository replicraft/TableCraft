mod error;
mod fields;
mod inspect;
mod metadata;
mod relationships;
mod rules;

pub use error::ConstraintViolation;
pub use inspect::check_table_constraints;
pub use metadata::{
    ConstraintReport, FieldCheck, RelationshipCheck, RelationshipCheckStatus, RuleCheckStatus,
};

#[cfg(feature = "ace16-write-preview")]
pub(crate) use inspect::check_scalar_projection;
