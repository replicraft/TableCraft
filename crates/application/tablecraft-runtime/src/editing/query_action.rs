//! Shared inspected-action facts; UPDATE and DELETE retain distinct capabilities.
use std::path::PathBuf;

pub(super) struct QueryActionTarget {
    pub(super) path: PathBuf,
    pub(super) document_revision: u64,
    pub(super) table: String,
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
pub(super) mod native;
