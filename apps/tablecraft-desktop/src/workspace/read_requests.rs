//! Translate permanent read identity exhaustion at the desktop boundary.
use super::{Desktop, PendingOperation};
use tablecraft_runtime::{Command, Error, ReadRequestExhausted, Worker};
use tablecraft_ui::State;

pub(super) fn request_read(command: Command, worker: &Worker, state: &mut State) -> Option<u64> {
    match worker.request(command) {
        Ok(id) => Some(id),
        Err(_) => {
            // No identity was issued. Revoke earlier publication rather than
            // allowing a late reply to overwrite the terminal restart guidance.
            exhausted(worker, state);
            None
        }
    }
}

/// A prepared UI token must be dropped before passing the state to this handler.
pub(super) fn exhausted(worker: &Worker, state: &mut State) {
    worker.cancel();
    state.busy = false;
    state.error = Some(state.localizer.text("objects-read-identities-exhausted"));
}

impl Desktop {
    /// Retire the earlier accepted UI request after its publication was revoked.
    /// This never releases a save actor or infers that a saved file is original.
    pub(super) fn read_request_rejected(&mut self) {
        self.fail_read(
            self.generation,
            &Error::ReadRequestExhausted(ReadRequestExhausted),
        );
        self.pending = PendingOperation::Read;
        self.state.busy = false;
        self.state.error = Some(
            self.state
                .localizer
                .text("objects-read-identities-exhausted"),
        );
    }
}

#[cfg(test)]
mod tests;
