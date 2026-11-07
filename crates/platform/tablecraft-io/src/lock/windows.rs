//! Windows can refuse an incompatible share mode before a lock is attempted.
use crate::IoError;
use std::io;

// WinError.h system error codes, independent of optional filesystem-journal FFI.
// https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--0-499-
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;

pub(super) fn open_error(error: io::Error) -> IoError {
    match error.raw_os_error() {
        Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION) => IoError::LockBusy,
        _ => error.into(),
    }
}

#[cfg(test)]
mod tests;
