//! Explicit restart attempts reuse the shared closed native recovery lifecycle.
use super::{
    RetainedOwner,
    outcomes::{failed_open, finish},
};
use crate::{
    controls::{SCOPE, action_recovery_budget, budget},
    editing::SaveOutcome,
};
use std::path::PathBuf;
use tablecraft_access::editing::offline::OfflineWriteSession;

pub(super) fn restart(
    source: PathBuf,
    bundle: PathBuf,
    retained: &mut Option<RetainedOwner>,
    cancelled: &dyn Fn() -> bool,
) -> SaveOutcome {
    let mut work = budget();
    let session = match OfflineWriteSession::open(&source, SCOPE, &mut work, cancelled) {
        Ok(session) => session,
        Err(error) => return failed_open(&error),
    };
    // Opening remains bounded by the ordinary native allowance. Journal image
    // validation and original-request reconstruction use their own finite pool.
    let mut recovery = action_recovery_budget();
    finish(
        session.recover_update(&bundle, &mut recovery, cancelled),
        retained,
    )
}
