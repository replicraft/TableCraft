//! Explicitly leave failed checked reopening only after native ownership ended.
use crate::workspace::{Desktop, PendingOperation};
use tablecraft_ui::FieldAdditionSaveIdentity;

impl Desktop {
    pub(in crate::workspace) fn return_to_field_addition_draft(
        &mut self,
        identity: FieldAdditionSaveIdentity,
    ) -> bool {
        let Some(host) = &self.editing.field_addition else {
            return false;
        };
        if host.identity != Some(identity)
            || host.reopening
            || self.editing.save.is_some()
            || self.editing.worker.is_busy()
            || !host.terminal.is_some_and(|outcome| {
                outcome
                    .field_addition_reopen_facts()
                    .is_ok_and(|facts| facts.replacement)
            })
            || !self.state.can_return_to_field_addition_draft(identity)
        {
            return false;
        }
        let cleanup = host.cleanup;
        if !self
            .state
            .return_field_addition_to_draft(&host.request, identity, cleanup)
        {
            return false;
        }
        if let Some(revision) = self
            .state
            .database
            .as_ref()
            .map(|database| database.document_revision)
        {
            self.state.set_save_cleanup(revision, cleanup);
        }
        self.editing.field_addition = None;
        self.cancel_table_draft_action();
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        true
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
