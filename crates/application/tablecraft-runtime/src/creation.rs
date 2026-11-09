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

/// The explicit fresh-native operation captured by a creation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationOperation {
    /// Create a new empty ACE 16 database without replacing an existing file.
    BlankAce16,
}

/// An explicit fresh destination, never an imported template or replacement grant.
pub struct CreationRequest {
    destination: PathBuf,
}

impl CreationRequest {
    /// Capture a fresh destination without opening it or taking a file lock.
    pub fn blank(destination: PathBuf) -> Self {
        Self { destination }
    }

    /// The exact destination supplied by the host, without path normalization.
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    /// The operation this request asks the nonreplaceable worker to perform.
    pub const fn operation(&self) -> CreationOperation {
        CreationOperation::BlankAce16
    }
}

/// Reserved before publication. Only completed reports are exposed by the worker.
pub struct CreationReport {
    /// Worker-local identity retained until explicit terminal acknowledgement.
    pub id: u64,
    /// The original requested creation operation.
    pub operation: CreationOperation,
    destination: PathBuf,
    outcome: Option<CreationOutcome>,
}

impl CreationReport {
    /// The destination captured before the actor attempted publication.
    pub fn destination(&self) -> &Path {
        &self.destination
    }

    /// The retained terminal outcome, including any publication uncertainty.
    ///
    /// # Panics
    ///
    /// Panics only if an internal invariant exposes an unpublished report.
    /// Public worker polling returns terminal reports exclusively.
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

/// Why the worker refused a request before accepting filesystem work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationStartFailure {
    /// The OS refused the worker thread before any request was accepted.
    ThreadStart(std::io::ErrorKind),
    /// Another operation or its unacknowledged terminal report owns the worker.
    Busy,
    /// No further worker-local request identity can be issued.
    Exhausted,
    /// The current target has no supported fresh-publication implementation.
    UnsupportedPlatform,
}

/// A rejected start returns the unconsumed destination request to its host.
pub struct CreationStartError {
    /// The refusal class; the request has not been queued or published.
    pub kind: CreationStartFailure,
    request: CreationRequest,
}

impl CreationStartError {
    /// Recover the exact original destination request after refusal.
    /// No creation work has been accepted for this returned request.
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
