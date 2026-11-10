//! Renderer-neutral desktop update status and explicit host-owned user intents.
mod restart;
mod state;
mod view;

/// A translated update failure without backend errors, paths or download URLs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateFailure {
    Network,
    Signature,
    Storage,
    InvalidRelease,
    InvalidArchive,
    Installation,
    NotConfigured,
    UnsupportedPlatform,
}

/// Why this installation requires a deliberate manual update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateManualReason {
    DevelopmentBuild,
    ManagedInstallation,
    UnrecognizedInstallation,
    UnsupportedPlatform,
}

/// Presentation facts supplied by the host's updater worker.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    Current,
    Available {
        version: String,
    },
    Downloading {
        received: u64,
        total: u64,
    },
    Ready {
        version: String,
    },
    Installing,
    Cancelling,
    Failed(UpdateFailure),
    Manual {
        version: Option<String>,
        reason: UpdateManualReason,
    },
}

#[derive(Default)]
pub(super) struct Model {
    available: bool,
    can_install: bool,
    status: UpdateStatus,
    generation: u64,
}

#[derive(Clone, Copy)]
enum Request {
    Check,
    Download,
    Cancel,
    Install,
}

#[cfg(test)]
mod tests;
