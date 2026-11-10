//! Windows Installer ownership and signed native package updates.

mod cabinet;
mod handoff;
mod installed;
mod native;
mod preparation;
mod report;

pub use installed::MsiInstallation;
#[cfg(windows)]
pub(crate) use installed::{detect, hint};
pub(crate) use preparation::PreparedMsi;

#[cfg(test)]
mod tests;
