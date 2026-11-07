//! Source-free table authoring. Draft identifiers never identify native records.
mod model;

pub use model::{DraftField, DraftFieldId, DraftFieldType, TableDraft};

use thiserror::Error;

pub const MAX_DRAFT_FIELDS: usize = 16;
pub const MAX_DRAFT_NAME_UNITS: usize = 64;
pub const SHORT_TEXT_UNITS: u16 = 255;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum TableDraftError {
    #[error("the draft field name is invalid or duplicated")]
    InvalidName,
    #[error("the draft field no longer exists")]
    MissingField,
    #[error("the initial AutoNumber field cannot be changed in this authoring profile")]
    ProtectedField,
    #[error("the table draft exceeds the bounded authoring profile")]
    Limit,
    #[error("the table draft allocation failed")]
    Allocation,
}

#[cfg(test)]
mod tests;
