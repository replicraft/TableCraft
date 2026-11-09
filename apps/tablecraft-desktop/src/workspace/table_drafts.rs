//! Retain a requested host transition while shared UI confirms draft disposal.
mod field_addition;
mod field_rename;
use super::Desktop;
use egui::{Context, ViewportCommand};
use tablecraft_runtime::Command;
use tablecraft_ui::{CreationRequest, ObjectTab};

pub(super) enum DeferredDraftAction {
    Open(Box<Command>),
    Create(CreationRequest),
    CloseWindow,
    CloseObject(tablecraft_ui::ObjectTab),
    ActivateObject(tablecraft_ui::ObjectTab),
}

impl Desktop {
    pub(super) fn guard_table_draft_action(
        &mut self,
        action: DeferredDraftAction,
    ) -> Option<DeferredDraftAction> {
        if self.deferred_draft_action.is_some() {
            drop(action);
            return None;
        }
        if self.state.table_draft_save_open()
            || self.state.table_draft_field_delete_open()
            || self.state.saved_query_sql_close_open()
            || self.state.field_rename_close_open()
        {
            drop(action);
            return None;
        }
        let action = self.defer_field_addition_transition(action)?;
        let action = self.defer_field_rename_transition(action)?;
        let confirmation = if self.state.has_modified_saved_query_sql() {
            self.state.request_saved_query_sql_close_all()
        } else if self.state.has_modified_table_drafts() {
            self.state.request_table_draft_discard()
        } else {
            return Some(action);
        };
        if !confirmation {
            drop(action);
            return None;
        }

        self.deferred_draft_action = Some(action);
        None
    }

    pub(super) fn resume_table_draft_action(&mut self, context: &Context) {
        let Some(action) = self.deferred_draft_action.take() else {
            return;
        };
        // Each decision resolves one exact owner. Retain the transition while
        // another saved SQL or table draft still needs its own confirmation.
        let Some(action) = self.guard_table_draft_action(action) else {
            return;
        };

        match action {
            DeferredDraftAction::Open(command) => self.open_request(*command),
            DeferredDraftAction::Create(request) => self.create_database(request),
            DeferredDraftAction::CloseWindow => context.send_viewport_cmd(ViewportCommand::Close),
            DeferredDraftAction::CloseObject(original) => {
                if let Some(tab) = self.preserved_transition_tab(&original) {
                    self.close_object(tab);
                }
            }
            DeferredDraftAction::ActivateObject(original) => {
                if let Some(tab) = self.preserved_transition_tab(&original) {
                    self.activate_object(tab);
                }
            }
        }
    }

    /// Rebase only preserved tab IDs/kinds after a checked replacement. No old
    /// row, native handle, query capability or field identity is revived here.
    pub(super) fn preserved_transition_tab(&self, original: &ObjectTab) -> Option<ObjectTab> {
        let revision = self.state.database.as_ref()?.document_revision;
        self.state.object_tabs().into_iter().find(|tab| {
            tab.id == original.id
                && tab.target.kind == original.target.kind
                && tab.target.document_revision == revision
        })
    }

    pub(super) fn cancel_table_draft_action(&mut self) {
        if let Some(action) = self.deferred_draft_action.take() {
            drop(action);
        }
    }

    pub(super) fn guard_table_draft_window_close(&mut self, context: &Context) {
        if self.editing.has_save() || self.creating.is_busy() {
            return;
        }
        if !context.input(|input| input.viewport().close_requested()) {
            return;
        }
        if self.deferred_draft_action.is_some()
            || self.state.table_draft_save_open()
            || self.state.table_draft_field_delete_open()
            || self.state.saved_query_sql_close_open()
            || self.state.field_rename_close_open()
        {
            context.send_viewport_cmd(ViewportCommand::CancelClose);
            return;
        }
        if self.state.has_modified_table_drafts()
            || self.state.has_modified_saved_query_sql()
            || self.state.field_addition_open()
            || self.state.field_rename_open()
        {
            context.send_viewport_cmd(ViewportCommand::CancelClose);
            let _ = self.guard_table_draft_action(DeferredDraftAction::CloseWindow);
        }
    }
}

#[cfg(test)]
mod tests;
