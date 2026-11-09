//! Source-neutral graph reconstruction; operation owners retain native authority.
mod admission;
mod capture;
mod publication;

pub(super) use admission::fits;
pub(super) use capture::prepare;
pub(super) use publication::publish;

use super::boundary;
use crate::workspace::{State, objects::MAX_RETAINED_BYTES};

/// Validate source-neutral owners and pending state; operation admission is separate.
pub(super) fn ready(state: &State) -> bool {
    let Some(database) = &state.database else {
        return false;
    };
    state
        .objects
        .entries
        .iter()
        .enumerate()
        .all(|(position, entry)| {
            boundary::bound(database, &entry.tab.target)
                && state.objects.entries[..position]
                    .iter()
                    .all(|old| old.tab.id != entry.tab.id && old.tab.target != entry.tab.target)
        })
        && state
            .objects
            .navigation
            .as_ref()
            .is_none_or(|target| boundary::bound(database, target))
        && boundary::current_optional(state)
        && !state.has_unsaved_table_drafts()
        && !state.has_modified_saved_query_sql()
        && state.table_draft.is_none()
        && !state.saved_query_sql_close_open()
        && !state.field_addition_close_open()
        && state.saved_query_sql_save_status().is_none()
        && state.table_draft_close.is_none()
        && !state.zoom_dialog_open()
        && !state.edit_dialog_open()
        && !state.insertion_dialog_open()
        && !state.deletion_dialog_open()
        && !state.recovery_prompt_open()
        && state.contents_request_id().is_none()
        && !state.parameter_prompt_open()
        && !state.query_action_dialog_open()
        && !state.table_draft_save_open()
        && !state.table_draft_field_delete_open()
        && state.query_sql_revision.checked_add(1).is_some()
        && state
            .object_bytes()
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
}

/// Source-neutral transfer choice; the operation separately validates approval.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Transfer {
    KeepAll,
    ClosePersisted,
}

impl Transfer {
    fn retains(self, kind: &crate::workspace::ObjectKind) -> bool {
        self == Self::KeepAll || matches!(kind, crate::workspace::ObjectKind::Sql)
    }
}
