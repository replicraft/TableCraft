//! Explicit fresh-copy publication with a typed platform durability guarantee.
//!
//! This is not a database transaction or a normal native-save permission. The
//! Unix backend anchors staging, no-clobber publication and cleanup to directory
//! handles. A receipt names that captured parent, not the current interpretation
//! of a pathname. Callers requiring stable textual destinations must separately
//! maintain their directory namespace. Callers must prevent replacement of the
//! operation's private staging entries by other actors, including writers to a
//! shared non-sticky parent. Descriptor anchoring and identity checks do not
//! establish that namespace precondition.
//! Windows uses qualified local NTFS write-through handle-relative rename and
//! a terminal file flush, not a Unix parent-directory barrier. Unsupported
//! platforms and filesystems refuse rather than downgrade the operation.

mod error;
#[cfg(any(unix, windows, test))]
mod protocol;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(any(windows, test))]
mod windows_names;

pub use error::{ParentIdentity, PublicationError, PublicationGuarantee, PublicationReceipt};

use std::path::Path;
use tablecraft_core::limits::ResourceBudget;

/// An owned parent capability for a single explicitly required publication.
/// Dropping it does not publish, delete a destination, or clean unknown entries.
pub struct RequiredOutput {
    #[cfg(unix)]
    backend: unix::Unix,
    #[cfg(windows)]
    backend: windows::Windows,
}

impl RequiredOutput {
    /// Validate the final component and capture its parent. Unix preflights the
    /// parent barrier; Windows qualifies its local NTFS handle-relative route.
    /// No temporary or final entry is created here. The receipt records the
    /// distinct platform guarantee; unsupported backends do not downgrade.
    pub fn capture(
        output: &Path,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PublicationError> {
        let _span = tracing::debug_span!("io.capture_required_output").entered();
        #[cfg(unix)]
        {
            Ok(Self {
                backend: unix::Unix::capture(output, budget, cancelled)?,
            })
        }
        #[cfg(windows)]
        {
            Ok(Self {
                backend: windows::Windows::capture(output, budget, cancelled)?,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (output, budget, cancelled);
            Err(PublicationError::UnsupportedDurability)
        }
    }

    /// Charge the supplied payload once, stage and sync it, then publish without
    /// clobbering. After known publication, finish cleanup and the reserved
    /// platform durability step without cancellation polls. Failures can retain
    /// a visible output; inspect the typed state and never retry or remove the
    /// final automatically.
    pub fn publish(
        self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PublicationReceipt, PublicationError> {
        let _span = tracing::debug_span!("io.publish_required_output").entered();
        #[cfg(any(unix, windows))]
        {
            protocol::publish(self.backend, bytes, budget, cancelled)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (bytes, budget, cancelled);
            Err(PublicationError::UnsupportedDurability)
        }
    }
}

#[cfg(test)]
mod tests;
