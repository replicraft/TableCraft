//! Retain a requested host transition while shared UI confirms draft disposal.
use super::Desktop;
use egui::{Context, ViewportCommand};
use tablecraft_runtime::Command;
use tablecraft_ui::CreationRequest;

pub(super) enum DeferredDraftAction {
    Open(Box<Command>),
    Create(CreationRequest),
    CloseWindow,
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
        if self.state.table_draft_save_open() || self.state.table_draft_field_delete_open() {
            drop(action);
            return None;
        }
        if !self.state.has_modified_table_drafts() {
            return Some(action);
        }
        if !self.state.request_table_draft_discard() {
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
        // Confirmation is only a host transition after the shared draft owner
        // has actually discarded its modified state.
        if self.state.has_modified_table_drafts() {
            drop(action);
            return;
        }

        match action {
            DeferredDraftAction::Open(command) => self.open_request(*command),
            DeferredDraftAction::Create(request) => self.create_database(request),
            DeferredDraftAction::CloseWindow => context.send_viewport_cmd(ViewportCommand::Close),
        }
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
        {
            context.send_viewport_cmd(ViewportCommand::CancelClose);
            return;
        }
        if self.state.has_modified_table_drafts() {
            context.send_viewport_cmd(ViewportCommand::CancelClose);
            let _ = self.guard_table_draft_action(DeferredDraftAction::CloseWindow);
        }
    }
}

#[cfg(test)]
mod tests;
