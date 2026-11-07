//! Retain actual publication facts and errors without granting retry or cleanup.
use std::{error::Error as StdError, fmt};
use tablecraft_access::creation::{CreationError, CreationReceipt as NativeReceipt};
use tablecraft_ace::{CreateDatabaseError, InspectError};
use tablecraft_io::{
    IoError,
    publication::{ParentIdentity, PublicationError, PublicationGuarantee},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationPhase {
    Capture,
    Construct,
    Publish,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationPublication {
    NotPublished { temporary_may_remain: bool },
    VisibilityUncertain,
    PublishedUnconfirmed,
    PublishedCleanupFailure { namespace_synced: bool },
}

/// Acknowledged native image publication under its explicit platform guarantee.
pub struct CreationReceipt {
    native: NativeReceipt,
}

impl CreationReceipt {
    pub fn table_count(&self) -> usize {
        self.native.table_count()
    }

    pub fn column_count(&self) -> usize {
        self.native.column_count()
    }

    pub fn row_count(&self) -> usize {
        self.native.row_count()
    }

    pub fn guarantee(&self) -> PublicationGuarantee {
        self.native.guarantee()
    }

    pub fn namespace_synced(&self) -> bool {
        self.native.namespace_synced()
    }

    pub fn parent_identity(&self) -> &ParentIdentity {
        self.native.parent_identity()
    }

    pub(super) fn new(native: NativeReceipt) -> Self {
        Self { native }
    }
}

impl fmt::Debug for CreationReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreationReceipt")
            .field("tables", &self.table_count())
            .field("columns", &self.column_count())
            .field("rows", &self.row_count())
            .field("guarantee", &self.guarantee())
            .field("namespace_synced", &self.namespace_synced())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum CreationOutcome {
    Created(CreationReceipt),
    Failed(CreationFailure),
}

enum Cause {
    Native(CreationError),
    WorkerPanicked,
}

/// The exact native error remains owned through every repeated report poll.
/// Visibility uncertainty is not a rollback, retry or final-file removal grant.
pub struct CreationFailure {
    pub phase: Option<CreationPhase>,
    pub publication: CreationPublication,
    cause: Cause,
}

impl CreationFailure {
    pub fn is_cancelled(&self) -> bool {
        match &self.cause {
            Cause::Native(CreationError::Publication(PublicationError::NotPublished {
                cause,
                ..
            })) => {
                matches!(cause, IoError::Cancelled)
            }
            Cause::Native(CreationError::Create(CreateDatabaseError::Inspect(error))) => {
                matches!(error, InspectError::Cancelled)
            }
            _ => false,
        }
    }

    pub fn is_resource_limit(&self) -> bool {
        match &self.cause {
            Cause::Native(CreationError::Publication(PublicationError::NotPublished {
                cause,
                ..
            })) => {
                matches!(cause, IoError::Limit(_))
            }
            Cause::Native(CreationError::Create(CreateDatabaseError::Inspect(error))) => {
                matches!(error, InspectError::Limit(_))
            }
            _ => false,
        }
    }

    pub fn unsupported_durability(&self) -> bool {
        matches!(
            self.cause,
            Cause::Native(CreationError::Publication(
                PublicationError::UnsupportedDurability
            ))
        )
    }

    pub fn worker_panicked(&self) -> bool {
        matches!(self.cause, Cause::WorkerPanicked)
    }

    pub(super) fn native(phase: CreationPhase, error: CreationError) -> Self {
        let publication = match &error {
            CreationError::Create(_) => CreationPublication::NotPublished {
                temporary_may_remain: false,
            },
            CreationError::Publication(error) => match error {
                PublicationError::UnsupportedDurability => CreationPublication::NotPublished {
                    temporary_may_remain: false,
                },
                PublicationError::NotPublished {
                    temporary_may_remain,
                    ..
                } => CreationPublication::NotPublished {
                    temporary_may_remain: *temporary_may_remain,
                },
                PublicationError::VisibilityUncertain { .. } => {
                    CreationPublication::VisibilityUncertain
                }
                PublicationError::PublishedUnconfirmed { .. } => {
                    CreationPublication::PublishedUnconfirmed
                }
                PublicationError::PublishedCleanupFailure {
                    namespace_synced, ..
                } => CreationPublication::PublishedCleanupFailure {
                    namespace_synced: *namespace_synced,
                },
            },
        };
        Self {
            phase: Some(phase),
            publication,
            cause: Cause::Native(error),
        }
    }

    pub(super) fn panic() -> Self {
        Self {
            phase: None,
            publication: CreationPublication::VisibilityUncertain,
            cause: Cause::WorkerPanicked,
        }
    }
}

impl fmt::Debug for CreationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CreationFailure")
            .field("phase", &self.phase)
            .field("publication", &self.publication)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for CreationFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Cause::Native(error) => error.fmt(formatter),
            Cause::WorkerPanicked => {
                formatter.write_str("creation worker failed; publication visibility is uncertain")
            }
        }
    }
}

impl StdError for CreationFailure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.cause {
            Cause::Native(error) => Some(error),
            Cause::WorkerPanicked => None,
        }
    }
}

#[cfg(test)]
mod tests;
