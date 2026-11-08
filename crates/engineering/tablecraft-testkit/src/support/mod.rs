//! Shared scenario and observation helpers, enabled by their test consumers.
#[cfg(all(
    feature = "access-support",
    any(target_os = "macos", target_os = "linux", windows)
))]
pub mod access;
#[cfg(feature = "ace-support")]
pub mod ace;
#[cfg(feature = "cli-support")]
pub mod cli;
#[cfg(feature = "query-support")]
pub mod query;

#[cfg(feature = "journal-support")]
pub mod io;
