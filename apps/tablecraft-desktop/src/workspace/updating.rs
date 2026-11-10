use super::Desktop;
use crate::updating::Reply;
use tablecraft_ui::{UpdateFailure, UpdateManualReason, UpdateStatus};
use tablecraft_update::{
    UpdateError,
    installation::ManualReason,
    session::{UpdateCommand, UpdateEvent, UpdateTaskError},
};

impl Desktop {
    fn can_install_update(&self) -> bool {
        self.state.can_restart_for_update()
            && !self.file_dialog.is_open()
            && !self.editing.has_save()
            && !self.creating.is_busy()
            && self.recovery.is_none()
            && self.recovery_reopen.is_none()
            && self.table_creation.is_none()
            && self.table_rename.is_none()
            && self.deferred_draft_action.is_none()
            && self.update_restart.is_none()
            && !self.updates.installing()
    }

    pub(super) fn start_update(&mut self, command: UpdateCommand) {
        if matches!(command, UpdateCommand::Install) && !self.can_install_update() {
            self.state
                .set_update_status(UpdateStatus::Failed(UpdateFailure::Installation));
            return;
        }
        if !self.updates.start(command) {
            self.state
                .set_update_status(UpdateStatus::Failed(UpdateFailure::Storage));
        }
    }

    pub(super) fn poll_updates(&mut self, context: &egui::Context) {
        while let Some(reply) = self.updates.poll() {
            match reply {
                Reply::Event { event, .. } => self.publish_update(event, context),
                Reply::Failed { error, .. } => {
                    let status = match error {
                        UpdateTaskError::Operation(UpdateError::Cancelled) => UpdateStatus::Idle,
                        UpdateTaskError::Operation(error) => UpdateStatus::Failed(failure(&error)),
                        UpdateTaskError::Apply(error) => {
                            // Keep explicit recovery ownership for diagnostic access;
                            // dropping the host must never delete retained originals.
                            self.update_failure = Some(error);
                            UpdateStatus::Failed(UpdateFailure::Installation)
                        }
                        UpdateTaskError::InvalidState => {
                            UpdateStatus::Failed(UpdateFailure::Installation)
                        }
                        _ => UpdateStatus::Failed(UpdateFailure::Installation),
                    };
                    self.state.set_update_status(status);
                }
            }
        }
        self.state.set_update_can_install(self.can_install_update());
        if self.updates.installing() && context.input(|input| input.viewport().close_requested()) {
            context.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }

    fn publish_update(&mut self, event: UpdateEvent, context: &egui::Context) {
        let status = match event {
            UpdateEvent::Checking => UpdateStatus::Checking,
            UpdateEvent::Current => UpdateStatus::Current,
            UpdateEvent::Available { version } => UpdateStatus::Available { version },
            UpdateEvent::Manual { version, reason } => UpdateStatus::Manual {
                version,
                reason: match reason {
                    ManualReason::DevelopmentBuild => UpdateManualReason::DevelopmentBuild,
                    ManualReason::UnsupportedLocation => UpdateManualReason::ManagedInstallation,
                    ManualReason::UnrecognizedLayout
                    | ManualReason::MissingManifest
                    | ManualReason::InvalidManifest => UpdateManualReason::UnrecognizedInstallation,
                },
            },
            UpdateEvent::Downloading(progress) => UpdateStatus::Downloading {
                received: progress.received,
                total: progress.total,
            },
            UpdateEvent::Ready { version } => UpdateStatus::Ready { version },
            UpdateEvent::Installing => UpdateStatus::Installing,
            UpdateEvent::RestartScheduled(applied) => {
                self.update_restart = Some(applied);
                context.send_viewport_cmd(egui::ViewportCommand::Close);
                UpdateStatus::Installing
            }
        };
        self.state.set_update_status(status);
    }
}

fn failure(error: &UpdateError) -> UpdateFailure {
    match error {
        UpdateError::NotConfigured | UpdateError::InvalidKey => UpdateFailure::NotConfigured,
        UpdateError::Network
        | UpdateError::HttpStatus
        | UpdateError::NotFound
        | UpdateError::UnauthorizedUrl => UpdateFailure::Network,
        UpdateError::Signature | UpdateError::PayloadChanged => UpdateFailure::Signature,
        UpdateError::ReleaseMetadata
        | UpdateError::ReleaseTooLarge
        | UpdateError::AssetTooLarge
        | UpdateError::LengthMismatch => UpdateFailure::InvalidRelease,
        UpdateError::Archive => UpdateFailure::InvalidArchive,
        UpdateError::Io(_) => UpdateFailure::Storage,
        _ => UpdateFailure::Installation,
    }
}

#[cfg(test)]
mod tests;
