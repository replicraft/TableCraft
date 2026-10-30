//! Explicit fresh-copy publication requiring a captured-parent namespace barrier.
//!
//! This is not a database transaction or a normal native-save permission. The
//! Unix backend anchors staging, no-clobber publication and cleanup to directory
//! handles. A receipt names that captured parent, not the current interpretation
//! of a pathname. Callers requiring stable textual destinations must separately
//! maintain their directory namespace. Callers must prevent replacement of the
//! operation's private staging entries by other actors, including writers to a
//! shared non-sticky parent. Descriptor anchoring and identity checks do not
//! establish that namespace precondition.
//! Default publication is unchanged; unsupported required backends fail early.

mod error;
#[cfg(any(unix, test))]
mod protocol;
#[cfg(unix)]
mod unix;

pub use error::{ParentIdentity, PublicationError, PublicationReceipt};

use std::path::Path;
use tablecraft_core::limits::ResourceBudget;

/// An owned parent capability for a single explicitly required publication.
/// Dropping it does not publish, delete a destination, or clean unknown entries.
pub struct RequiredOutput {
    #[cfg(unix)]
    backend: unix::Unix,
}

impl RequiredOutput {
    /// Validate the final component, capture its parent and preflight its barrier.
    /// No temporary or final entry is created here. Windows currently refuses
    /// required durability; ordinary host publication remains available.
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
        #[cfg(not(unix))]
        {
            let _ = (output, budget, cancelled);
            Err(PublicationError::UnsupportedDurability)
        }
    }

    /// Charge the supplied payload once, stage and sync it, then publish without
    /// clobbering. After known publication, finish cleanup and the reserved parent
    /// barrier without cancellation polls. Failures can retain a visible output;
    /// inspect the typed state and never retry or remove the final automatically.
    pub fn publish(
        self,
        bytes: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<PublicationReceipt, PublicationError> {
        let _span = tracing::debug_span!("io.publish_required_output").entered();
        #[cfg(unix)]
        {
            protocol::publish(self.backend, bytes, budget, cancelled)
        }
        #[cfg(not(unix))]
        {
            let _ = (bytes, budget, cancelled);
            Err(PublicationError::UnsupportedDurability)
        }
    }
}

#[cfg(test)]
mod tests;
