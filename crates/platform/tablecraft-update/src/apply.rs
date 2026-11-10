//! Native installation replacement with retained backups and deferred relaunch.

mod helper;
mod outcome;
mod transaction;

pub use outcome::{AppliedUpdate, ApplyError, RetainedRecovery};
pub(crate) use transaction::apply;

#[cfg(test)]
mod tests;
