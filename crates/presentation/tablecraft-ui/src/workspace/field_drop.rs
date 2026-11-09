//! Exact pending deletion presentation, separate from native DROP authority.
mod close;
pub(super) mod model;
mod preparation;
mod state;
mod status;
#[cfg(test)]
mod tests;
mod warning;

pub use close::{FieldDropCloseDecision, FieldDropCloseResult};
pub use preparation::{
    PreparedFieldDropInspection, PreparedFieldDropResolution, PreparedFieldDropSave,
};
pub use status::FieldDropSaveCommand;
pub use warning::FieldDropWarning;

use super::field_name::bounded_name;
use super::{ObjectKind, ObjectTab, ObjectTarget};

/// Exact persisted object owner and actual field metadata, never a Caption.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldDropTarget {
    pub tab: ObjectTab,
    pub column: String,
    pub native_number: usize,
}

/// Exact confirmed local deletion submitted only by explicit Save.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldDropRequest {
    pub target: FieldDropTarget,
    pub draft_revision: u64,
}

/// Operation identity exists only after the actual save actor accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldDropSaveIdentity {
    pub request_id: u64,
    pub review_id: u64,
    pub save_id: u64,
}

/// Local destructive confirmation, not native write permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldDropConfirmationDecision {
    Yes,
    No,
}

/// Lets a host revoke exactly the dismissed readonly inspection publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldDropConfirmationResult {
    pub decision: FieldDropConfirmationDecision,
    pub inspection_request_id: Option<u64>,
}

impl FieldDropTarget {
    pub(super) fn bounded(&self) -> bool {
        !self.column.is_empty()
            && bounded_name(&self.column)
            && matches!(&self.tab.target.kind, ObjectKind::Table { name }
                if !name.is_empty() && bounded_name(name))
    }

    pub(super) fn heap_bytes(&self) -> Option<usize> {
        let ObjectKind::Table { name } = &self.tab.target.kind else {
            return None;
        };
        name.capacity().checked_add(self.column.capacity())
    }

    pub(super) fn copy(&self) -> Option<Self> {
        if !self.bounded() {
            return None;
        }
        let ObjectKind::Table { name } = &self.tab.target.kind else {
            return None;
        };
        Some(Self {
            tab: ObjectTab {
                id: self.tab.id,
                target: ObjectTarget {
                    document_revision: self.tab.target.document_revision,
                    kind: ObjectKind::Table {
                        name: copy_text(name)?,
                    },
                },
            },
            column: copy_text(&self.column)?,
            native_number: self.native_number,
        })
    }
}

impl FieldDropRequest {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        size_of::<Self>().checked_add(self.target.heap_bytes()?)
    }
}

fn copy_text(text: &str) -> Option<String> {
    if !bounded_name(text) {
        return None;
    }
    let mut copy = String::new();
    copy.try_reserve_exact(text.len()).ok()?;
    copy.push_str(text);
    Some(copy)
}
