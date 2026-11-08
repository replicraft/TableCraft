//! Source-free table authoring. Draft identifiers never identify native records.
//!
//! Text-only policies retain hidden editor caches across type changes; effective
//! getters and immutable snapshots expose them only for Short Text. Required
//! and default-mode choices persist across ordinary type changes without parsing
//! or conversion. Native consumers still validate all syntax and compatibility.
mod model;
mod snapshot;

pub use model::{
    DraftDefaultMode, DraftField, DraftFieldId, DraftFieldType, DraftNumberSize, TableDraft,
};
pub use snapshot::{SnapshotField, TableDraftSnapshot};

use thiserror::Error;

pub const MAX_DRAFT_FIELDS: usize = 16;
pub const MAX_DRAFT_NAME_UNITS: usize = 64;
pub const MAX_DRAFT_NAME_BYTES: usize = MAX_DRAFT_NAME_UNITS * 4;
pub const SHORT_TEXT_UNITS: u16 = 255;
pub const MIN_SHORT_TEXT_UNITS: u16 = 1;
pub const MAX_DRAFT_DEFAULT_BYTES: usize = 4096;
pub const MAX_DRAFT_DESCRIPTION_UNITS: usize = 255;
pub const MAX_DRAFT_DESCRIPTION_BYTES: usize = MAX_DRAFT_DESCRIPTION_UNITS * 4;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum TableDraftError {
    #[error("the draft name is invalid or duplicated")]
    InvalidName,
    #[error("the draft field description exceeds its bounded text profile")]
    InvalidDescription,
    #[error("the draft field no longer exists")]
    MissingField,
    #[error("the initial AutoNumber field cannot be changed in this authoring profile")]
    ProtectedField,
    #[error("the draft property is invalid for this field declaration")]
    InvalidProperty,
    #[error("the table draft exceeds the bounded authoring profile")]
    Limit,
    #[error("the table draft allocation failed")]
    Allocation,
}

#[cfg(test)]
mod tests;

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_DRAFT_NAME_BYTES
        && name.encode_utf16().take(MAX_DRAFT_NAME_UNITS + 1).count() <= MAX_DRAFT_NAME_UNITS
        && !name
            .chars()
            .any(|character| character.is_control() || ".!`[]".contains(character))
        && name.trim() == name
}
