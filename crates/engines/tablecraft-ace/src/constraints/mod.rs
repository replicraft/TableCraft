mod error;
mod fields;
mod inspect;
mod metadata;
mod relationships;

pub use error::ConstraintViolation;
pub use inspect::check_table_constraints;
pub use metadata::{
    ConstraintReport, FieldCheck, RelationshipCheck, RelationshipCheckStatus, RuleCheckStatus,
};
