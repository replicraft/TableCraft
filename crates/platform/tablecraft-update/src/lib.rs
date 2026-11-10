//! Signed desktop updates, independent of rendering and database ownership.
//!
//! Operations block and belong on a host-owned worker. Production trust is
//! compiled from the maintained public key; network metadata never selects it.

pub mod apply;
mod archive;
pub mod artifact;
mod client;
mod download;
mod error;
mod http;
pub mod installation;
mod package;
pub mod platform;
mod preparation;
pub mod release;
pub mod session;
mod trust;
mod verification;

pub use client::Updater;
pub use download::{DownloadProgress, VerifiedDownload};
pub use error::UpdateError;
pub use preparation::PreparedUpdate;
