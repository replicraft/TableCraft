//! Consuming update commands for a host-owned worker and terminal result channel.

use crate::{
    DownloadProgress, PreparedUpdate, UpdateError, Updater,
    apply::{AppliedUpdate, ApplyError},
    artifact::ArtifactKind,
    installation::{self, InstallPlan, ManualReason},
    platform::Platform,
    release::AvailableUpdate,
    target::UpdateTarget,
};
use semver::Version;
use std::mem;
use thiserror::Error;

#[cfg(windows)]
use crate::msi;

/// An explicitly requested operation on one worker-owned update session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateCommand {
    /// Check the fixed release endpoint and classify the local installation.
    Check,
    /// Download and prepare the exact update captured by a successful check.
    Download,
    /// Replace the installation and schedule relaunch after normal host shutdown.
    Install,
}

/// Update progress and consuming terminal results delivered to the host.
#[derive(Debug)]
pub enum UpdateEvent {
    /// A release check is about to begin.
    Checking,
    /// No newer matching release is available.
    Current,
    /// A newer release can update this recognized local installation.
    Available {
        /// The exact release version selected for this installation.
        version: String,
    },
    /// This installation needs a manual update instead of replacement.
    Manual {
        /// The newer release version, when the check found one.
        version: Option<String>,
        /// Why this local installation cannot be replaced automatically.
        reason: ManualReason,
    },
    /// Archive download progress; the archive is not yet accepted for installation.
    Downloading(DownloadProgress),
    /// Authentication and extraction finished without changing installed files.
    Ready {
        /// The authenticated release version held by the prepared capability.
        version: String,
    },
    /// A prepared update is about to enter its non-cancellable replacement phase.
    Installing,
    /// Replacement succeeded and the helper now awaits normal host shutdown.
    ///
    /// This is a terminal ownership transfer. A closed presentation channel must
    /// not turn it into a cancellation or discard retained recovery information.
    RestartScheduled(AppliedUpdate),
}

/// A command failure distinct from presentation events and installed outcomes.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UpdateTaskError {
    /// Checking, downloading or preparing failed before installation changed.
    #[error("The update operation failed")]
    Operation(#[from] UpdateError),
    /// Installation failed, retaining recovery resources when necessary.
    #[error("The update installation failed")]
    Apply(#[from] ApplyError),
    /// The command did not have the preceding checked or prepared capability.
    #[error("The update command is not available in the current state")]
    InvalidState,
}

/// A synchronous command session owned entirely by one host worker.
///
/// No method spawns a worker, starts background checks or opens a database. Hosts
/// must resolve unsaved documents and active save/recovery workers before sending
/// `Install`, then quit normally only after `RestartScheduled` is retained.
#[derive(Debug)]
pub struct Session {
    updater: Updater,
    state: State,
}

#[derive(Debug)]
enum State {
    Idle,
    Available(Box<AvailableState>),
    Ready(PreparedUpdate),
    RestartScheduled,
}

#[derive(Debug)]
struct AvailableState {
    release: AvailableUpdate,
    target: UpdateTarget,
}

impl Session {
    /// Creates a session using compiled publisher trust and the fixed release endpoint.
    ///
    /// # Errors
    /// Reports missing or invalid publisher trust and HTTP client initialization
    /// failures. Construction and subsequent commands belong on the host worker.
    pub fn new(version: Version, platform: Platform) -> Result<Self, UpdateError> {
        Ok(Self {
            updater: Updater::new(version, platform)?,
            state: State::Idle,
        })
    }

    /// Executes one explicit update command and emits progress to its owning host.
    ///
    /// Returning `false` from `emit` cancels checks, downloads and preparation.
    /// Once replacement succeeds, the terminal `RestartScheduled` event cannot
    /// be revoked: its callback return value is ignored. The host must retain
    /// terminal outcomes even when its UI channel closes. No later command can
    /// reuse the session after a successful replacement.
    ///
    /// # Errors
    /// Reports invalid command ordering, cancellation, check/download/preparation
    /// failures and typed installation failures. A failed check or download clears
    /// captured capabilities; it does not alter installed application files.
    pub fn execute(
        &mut self,
        command: UpdateCommand,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(UpdateEvent) -> bool,
    ) -> Result<(), UpdateTaskError> {
        if matches!(self.state, State::RestartScheduled) {
            return Err(UpdateTaskError::InvalidState);
        }

        let result = match command {
            UpdateCommand::Check => self.check(cancelled, emit),
            UpdateCommand::Download => self.download(cancelled, emit),
            UpdateCommand::Install => self.install(cancelled, emit),
        };
        if result.is_err() && matches!(command, UpdateCommand::Check | UpdateCommand::Download) {
            self.state = State::Idle;
        }
        result
    }

    fn check(
        &mut self,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(UpdateEvent) -> bool,
    ) -> Result<(), UpdateTaskError> {
        self.state = State::Idle;
        publish(UpdateEvent::Checking, cancelled, emit)?;
        #[cfg(windows)]
        if msi::hint(self.updater.platform())? {
            let Some(release) = self.updater.check(ArtifactKind::WindowsMsi, cancelled)? else {
                return publish(UpdateEvent::Current, cancelled, emit);
            };
            let installed =
                msi::detect(self.updater.version(), self.updater.platform(), cancelled)?;
            return self.accept_target(
                release,
                UpdateTarget::WindowsMsi(installed),
                cancelled,
                emit,
            );
        }
        let available = self
            .updater
            .check(ArtifactKind::PortableArchive, cancelled)?;
        let outcome = classify_checked(available, || {
            installation::detect(self.updater.version(), self.updater.platform())
        })?;
        self.accept_checked(outcome, cancelled, emit)
    }

    fn accept_checked(
        &mut self,
        outcome: Option<(AvailableUpdate, InstallPlan)>,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(UpdateEvent) -> bool,
    ) -> Result<(), UpdateTaskError> {
        let Some((release, installation)) = outcome else {
            return publish(UpdateEvent::Current, cancelled, emit);
        };
        match installation {
            InstallPlan::Manual(reason) => publish(
                UpdateEvent::Manual {
                    version: Some(release.version().to_string()),
                    reason,
                },
                cancelled,
                emit,
            ),
            InstallPlan::Portable(installed) | InstallPlan::MacBundle(installed) => {
                self.accept_target(release, UpdateTarget::Package(installed), cancelled, emit)
            }
        }
    }

    fn accept_target(
        &mut self,
        release: AvailableUpdate,
        target: UpdateTarget,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(UpdateEvent) -> bool,
    ) -> Result<(), UpdateTaskError> {
        if release.kind() != target.kind() || release.platform() != target.platform() {
            return Err(UpdateError::ReleaseMetadata.into());
        }
        let version = release.version().to_string();
        publish(UpdateEvent::Available { version }, cancelled, emit)?;
        self.state = State::Available(Box::new(AvailableState { release, target }));
        Ok(())
    }

    fn download(
        &mut self,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(UpdateEvent) -> bool,
    ) -> Result<(), UpdateTaskError> {
        let State::Available(available) = mem::replace(&mut self.state, State::Idle) else {
            return Err(UpdateTaskError::InvalidState);
        };
        let AvailableState { release, target } = *available;
        check_cancelled(cancelled)?;
        let verified =
            self.updater
                .download(&release, target.staging_parent(), &mut |progress| {
                    !cancelled() && emit(UpdateEvent::Downloading(progress))
                })?;
        let prepared = verified.prepare(target, cancelled)?;
        let version = prepared.version().to_string();
        publish(UpdateEvent::Ready { version }, cancelled, emit)?;
        self.state = State::Ready(prepared);
        Ok(())
    }

    fn install(
        &mut self,
        cancelled: &dyn Fn() -> bool,
        emit: &mut dyn FnMut(UpdateEvent) -> bool,
    ) -> Result<(), UpdateTaskError> {
        if !matches!(self.state, State::Ready(_)) {
            return Err(UpdateTaskError::InvalidState);
        }
        let State::Ready(prepared) = mem::replace(&mut self.state, State::Idle) else {
            return Err(UpdateTaskError::InvalidState);
        };
        publish(UpdateEvent::Installing, cancelled, emit)?;
        check_cancelled(cancelled)?;
        let applied = prepared.apply(cancelled)?;
        self.state = State::RestartScheduled;
        // Replacement has already happened. Never report cancellation because
        // a presentation receiver disappears after this terminal transition.
        let _ = emit(UpdateEvent::RestartScheduled(applied));
        Ok(())
    }
}

fn classify_checked(
    available: Option<AvailableUpdate>,
    detect: impl FnOnce() -> Result<InstallPlan, UpdateError>,
) -> Result<Option<(AvailableUpdate, InstallPlan)>, UpdateError> {
    let Some(release) = available else {
        return Ok(None);
    };
    Ok(Some((release, detect()?)))
}

fn publish(
    event: UpdateEvent,
    cancelled: &dyn Fn() -> bool,
    emit: &mut dyn FnMut(UpdateEvent) -> bool,
) -> Result<(), UpdateTaskError> {
    check_cancelled(cancelled)?;
    if emit(event) {
        Ok(())
    } else {
        Err(UpdateError::Cancelled.into())
    }
}

fn check_cancelled(cancelled: &dyn Fn() -> bool) -> Result<(), UpdateError> {
    if cancelled() {
        Err(UpdateError::Cancelled)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
