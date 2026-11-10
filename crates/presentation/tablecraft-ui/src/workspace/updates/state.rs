use super::{Request, UpdateStatus};
use crate::workspace::{Action, State};

impl State {
    /// Enable update intents only after the host has connected its worker.
    pub fn set_updates_available(&mut self, available: bool) {
        self.updates.available = available;
    }

    /// Allow installation only when the host has resolved edits and native workers.
    ///
    /// This is a presentation gate; the installer must recheck ownership itself.
    pub fn set_update_can_install(&mut self, can_install: bool) {
        self.updates.can_install = can_install;
    }

    /// Publish a worker observation after the host checks its current operation identity.
    pub fn set_update_status(&mut self, status: UpdateStatus) {
        self.updates.status = status;
    }

    /// The current renderer-neutral update observation.
    pub fn update_status(&self) -> &UpdateStatus {
        &self.updates.status
    }

    pub(super) fn request_update(&mut self, request: Request) -> Option<Action> {
        if !self.updates.available || self.mock_preview {
            return None;
        }
        let (action, status) = match (request, &self.updates.status) {
            (
                Request::Check,
                UpdateStatus::Idle
                | UpdateStatus::Current
                | UpdateStatus::Failed(_)
                | UpdateStatus::Manual { .. },
            ) => (Action::CheckForUpdates, UpdateStatus::Checking),
            (Request::Download, UpdateStatus::Available { .. }) => (
                Action::DownloadUpdate,
                UpdateStatus::Downloading {
                    received: 0,
                    total: 0,
                },
            ),
            (Request::Cancel, UpdateStatus::Checking | UpdateStatus::Downloading { .. }) => {
                (Action::CancelUpdate, UpdateStatus::Cancelling)
            }
            (Request::Install, UpdateStatus::Ready { .. })
                if self.updates.can_install && self.can_restart_for_update() =>
            {
                (Action::InstallUpdate, UpdateStatus::Installing)
            }
            _ => return None,
        };
        // Change the visible phase before dispatch so another click cannot issue
        // a duplicate request while the host is accepting the first command.
        let generation = self.updates.generation.checked_add(1)?;
        self.updates.generation = generation;
        self.updates.status = status;
        Some(action)
    }
}
