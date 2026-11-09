//! Dispatch explicit global-record requests through the existing page service.
use super::{Desktop, PendingOperation};
use crate::workspace::request_read;
use tablecraft_runtime::Command;
use tablecraft_ui::RecordTarget;

impl Desktop {
    pub(super) fn navigate_record(&mut self, target: RecordTarget) {
        if self.editing.has_save() || self.state.busy || !self.state.can_navigate_record(&target) {
            return;
        }

        let Some(request_id) = request_read(
            Command::Page {
                // NO_CLONE: The runtime Page command and shared/host RecordTarget independently
                // retain table identity across navigation completion.
                table: target.table.clone(),
                offset: target.parent_ordinal,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        self.pending = PendingOperation::Record {
            // NO_CLONE: Shared record-navigation pending and desktop pending each own the exact
            // target for source/offset validation and request cancellation.
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
