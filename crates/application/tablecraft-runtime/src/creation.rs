//! Fresh blank native creation is a separate, nonreplaceable application operation.
mod outcome;
mod service;
mod worker;

pub use outcome::{
    CreationFailure, CreationOutcome, CreationPhase, CreationPublication, CreationReceipt,
};
pub use worker::CreationWorker;

use std::{
    fmt,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationOperation {
    BlankAce16,
}

/// An explicit fresh destination, never an imported template or replacement grant.
pub struct CreationRequest {
    destination: PathBuf,
}

impl CreationRequest {
    pub fn blank(destination: PathBuf) -> Self {
        Self { destination }
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub const fn operation(&self) -> CreationOperation {
        CreationOperation::BlankAce16
    }
}

/// Reserved before publication. Only completed reports are exposed by the worker.
pub struct CreationReport {
    pub id: u64,
    pub operation: CreationOperation,
    destination: PathBuf,
    outcome: Option<CreationOutcome>,
}

impl CreationReport {
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub fn outcome(&self) -> &CreationOutcome {
        self.outcome
            .as_ref()
            .expect("only terminal creation reports are published")
    }
}

impl fmt::Debug for CreationReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreationReport")
            .field("id", &self.id)
            .field("operation", &self.operation)
            .field("outcome", &self.outcome)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationStartFailure {
    /// The OS refused the worker thread before any request was accepted.
    ThreadStart(std::io::ErrorKind),
    Busy,
    Exhausted,
    UnsupportedPlatform,
}

/// A rejected start returns the unconsumed destination request to its host.
pub struct CreationStartError {
    pub kind: CreationStartFailure,
    request: CreationRequest,
}

impl CreationStartError {
    pub fn into_request(self) -> CreationRequest {
        self.request
    }
}

impl fmt::Debug for CreationStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreationStartError")
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CreationStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "creation request was not accepted ({:?})",
            self.kind
        )
    }
}

impl std::error::Error for CreationStartError {}
