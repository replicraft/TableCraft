//! Explicit retained filesystem pairing for fixed V1 image journals.
//!
//! This owner establishes object association and exclusive handle retention,
//! not native permission, global identity, discovery or a replay-safe registry.
//! Callers retain offline/noncooperating-writer/private-namespace preconditions.
//! Initial filesystem support is macOS/Linux; unsupported Unix refuses before
//! opening source files. Executable hosts provide unsupported handlers elsewhere.
mod binding;
mod pair;
mod reopen;
mod storage;
mod target;

pub use binding::BindingError;
pub use pair::{OfflineJournalPair, PairError, PairSetupFailure, PairState};
pub use storage::PairedStorage;
pub use target::{OfflineTarget, TargetLockState};

use crate::IoError;
use std::ffi::CStr;
use std::io::ErrorKind;

const BINDING_ENTRY: &CStr = c"binding";
const INTENT_ENTRY: &CStr = c"intent";
const JOURNAL_ENTRY: &CStr = c"journal";

fn require_platform() -> Result<(), IoError> {
    if cfg!(any(target_os = "macos", target_os = "linux")) {
        Ok(())
    } else {
        Err(IoError::Io(ErrorKind::Unsupported))
    }
}

#[cfg(all(test, not(any(target_os = "macos", target_os = "linux"))))]
mod unsupported_tests;
