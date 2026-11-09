//! Closed-table deletion presentation retains intent, never native mutation authority.
mod dialog;
pub(super) mod model;
mod state;
mod status;
#[cfg(test)]
mod tests;

pub use status::TableDropSaveCommand;

use super::{ObjectKind, ObjectTarget};

/// Exact closed catalog object selected by the user for destructive confirmation.
/// The runtime must independently bind native identity, contents and dependencies.
#[derive(Debug, PartialEq, Eq)]
pub struct TableDropRequest {
    pub id: u64,
    pub target: ObjectTarget,
}

impl TableDropRequest {
    /// Original stored table spelling, if the caller supplied a table target.
    pub fn table_name(&self) -> Option<&str> {
        match &self.target.kind {
            ObjectKind::Table { name } => Some(name),
            _ => None,
        }
    }
}

/// Exact review and actor ownership, distinct from the presentation request ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableDropSaveIdentity {
    pub request_id: u64,
    pub review_id: u64,
    pub save_id: u64,
}
