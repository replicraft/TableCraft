//! Source-observed saved-field proposals, separate from consuming native authority.
mod close;
mod dialog;
#[cfg(test)]
mod entry_tests;
pub(crate) mod header;
#[cfg(test)]
mod interaction_tests;
pub(super) mod model;
pub(crate) mod rendering;
pub(crate) mod widget;
pub use close::{FieldRenameCloseDecision, FieldRenameCloseResult, PreparedCleanFieldRenameClose};
mod preparation;
mod selection;
mod state;
mod status;
mod warning;

#[cfg(test)]
mod model_tests;

pub use preparation::{
    PreparedFieldRenameInspection, PreparedFieldRenameResolution, PreparedFieldRenameSave,
    PreparedUnchangedFieldRename,
};
pub use status::FieldRenameSaveCommand;
pub use warning::FieldRenameWarning;

use super::{ObjectKind, ObjectTab};

/// Exact field metadata selected in one persisted native table session.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldRenameTarget {
    pub tab: ObjectTab,
    pub column: String,
    pub native_number: usize,
}

/// Exact text/revision snapshot captured after this frame's name widget.
#[derive(Debug, PartialEq, Eq)]
pub struct FieldRenameRequest {
    pub target: FieldRenameTarget,
    pub draft_revision: u64,
    pub new_name: String,
}

impl FieldRenameTarget {
    pub(super) fn bounded(&self) -> bool {
        crate::workspace::field_name::bounded_name(&self.column)
            && !self.column.is_empty()
            && matches!(&self.tab.target.kind, ObjectKind::Table { name }
                if !name.is_empty() && crate::workspace::field_name::bounded_name(name))
    }

    pub(super) fn heap_bytes(&self) -> Option<usize> {
        let ObjectKind::Table { name } = &self.tab.target.kind else {
            return None;
        };
        name.capacity().checked_add(self.column.capacity())
    }

    pub(super) fn copy(&self) -> Option<Self> {
        Self::from_parts(&self.tab, &self.column, self.native_number)
    }

    pub(crate) fn from_parts(tab: &ObjectTab, column: &str, native_number: usize) -> Option<Self> {
        let ObjectKind::Table { name } = &tab.target.kind else {
            return None;
        };
        if name.is_empty()
            || column.is_empty()
            || !crate::workspace::field_name::bounded_name(name)
            || !crate::workspace::field_name::bounded_name(column)
        {
            return None;
        }

        Some(Self {
            tab: ObjectTab {
                id: tab.id,
                target: super::ObjectTarget {
                    document_revision: tab.target.document_revision,
                    kind: ObjectKind::Table {
                        name: copy_text(name)?,
                    },
                },
            },
            column: copy_text(column)?,
            native_number,
        })
    }
}

impl FieldRenameRequest {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        size_of::<Self>()
            .checked_add(self.target.heap_bytes()?)?
            .checked_add(self.new_name.capacity())
    }
}

fn copy_text(text: &str) -> Option<String> {
    let mut copy = String::new();
    copy.try_reserve_exact(text.len()).ok()?;
    copy.push_str(text);
    Some(copy)
}

/// Exact operation ownership. An unchanged review has no native save actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldRenameSaveIdentity {
    NativeSave {
        request_id: u64,
        review_id: u64,
        save_id: u64,
    },
    UnchangedReview {
        request_id: u64,
        review_id: u64,
    },
}

#[cfg(test)]
mod state_tests;
#[cfg(test)]
pub(in crate::workspace) mod test_support;
