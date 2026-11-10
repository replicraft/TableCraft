//! Signed desktop updates, independent of rendering and database ownership.
//!
//! Operations block and belong on a host-owned worker. Production trust is
//! compiled from the maintained public key; network metadata never selects it.

mod client;
mod download;
mod error;
mod http;
pub mod platform;
pub mod release;
mod trust;
mod verification;

pub use client::Updater;
pub use download::{DownloadProgress, VerifiedDownload};
pub use error::UpdateError;
