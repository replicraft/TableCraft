//! Saved SQL text is object-owned presentation state, never native authority.
mod close;
mod draft;

pub(in crate::workspace) use close::Confirmation;
pub use close::{SavedQuerySqlCloseDecision, SavedQuerySqlCloseResult};
mod inspection;
mod lifecycle;
mod rebind;

pub(in crate::workspace) use rebind::{SqlResolution, copy as copy_rebind_text};
mod state;

pub(in crate::workspace) use draft::Draft;
pub use draft::{SavedQuerySqlEligibility, SavedQuerySqlError, SavedQuerySqlRequest};

#[cfg(test)]
mod tests;
