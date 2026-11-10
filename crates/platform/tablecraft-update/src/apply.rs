//! Native installation replacement with retained backups and deferred relaunch.

mod helper;
mod outcome;
pub(crate) mod transaction;

pub use outcome::{AppliedUpdate, ApplyError, RetainedRecovery};

#[cfg(test)]
mod tests;
