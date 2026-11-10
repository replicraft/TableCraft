use std::io;
use thiserror::Error;

/// A payload-free update failure suitable for host translation and reporting.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UpdateError {
    /// No production signing key was included in this build.
    #[error("Updates are unavailable: the release signing key is not configured")]
    NotConfigured,
    /// The maintained signing key is malformed.
    #[error("The release signing key is invalid")]
    InvalidKey,
    /// Release metadata failed structural or identity validation.
    #[error("The release metadata is invalid")]
    ReleaseMetadata,
    /// Release JSON exceeded the independent metadata allowance.
    #[error("The release metadata is too large")]
    ReleaseTooLarge,
    /// A release asset exceeded its independent allowance.
    #[error("The update asset is too large")]
    AssetTooLarge,
    /// An update attempted to leave the authorized HTTPS host set.
    #[error("The update URL is not authorized")]
    UnauthorizedUrl,
    /// Network failure; details intentionally exclude URLs and redirect tokens.
    #[error("The update server could not be reached")]
    Network,
    /// The fixed endpoint or selected asset does not exist.
    #[error("No matching release asset exists")]
    NotFound,
    /// The server returned an unsuccessful response.
    #[error("The update server returned an unexpected status")]
    HttpStatus,
    /// Downloaded bytes did not match the captured release size.
    #[error("The update download has an unexpected length")]
    LengthMismatch,
    /// Signature parsing, publisher authentication or trusted identity failed.
    #[error("The update signature is invalid")]
    Signature,
    /// The caller cancelled before publication of a verified update.
    #[error("The update was cancelled")]
    Cancelled,
    /// Staging or reading an owned local file failed.
    #[error("The update file could not be accessed")]
    Io(#[source] io::Error),
    /// A signed archive failed path, membership or content validation.
    #[error("The update archive is invalid")]
    Archive,
    /// The candidate installation has no authenticated replacement authority.
    #[error("The installed application could not be authenticated")]
    UnqualifiedInstallation,
    /// Staged bytes changed after verification.
    #[error("The staged update changed after signature verification")]
    PayloadChanged,
}
