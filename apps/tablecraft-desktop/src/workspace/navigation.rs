//! Dispatch explicit global-record requests through the existing page service.
use super::{Desktop, PendingOperation};
use tablecraft_runtime::Command;
use tablecraft_ui::RecordTarget;

impl Desktop {
    pub(super) fn navigate_record(&mut self, target: RecordTarget) {
        if self.editing.has_save() || self.state.busy || !self.state.can_navigate_record(&target) {
            return;
        }

        let request_id = self.worker.request(Command::Page {
            table: target.table.clone(),
            offset: target.parent_ordinal,
        });
        self.generation = request_id;
        self.pending = PendingOperation::Record {
            target: target.clone(),
            request_id,
        };
        self.state.begin_record_navigation(target, request_id);
        self.state.busy = true;
        self.state.error = None;
    }
}

#[cfg(test)]
mod tests;
