//! Sealed local installation proofs consumed by matching signed payload routes.

use crate::{
    artifact::ArtifactKind, installation::Installed, msi::MsiInstallation, platform::Platform,
};
use semver::Version;
use std::path::Path;

/// A local target whose owning subsystem has established update authority.
#[derive(Debug)]
pub enum UpdateTarget {
    /// A manifest-owned portable directory or macOS bundle.
    Package(Installed),
    /// A Windows Installer product bound through native registration and components.
    WindowsMsi(MsiInstallation),
}

impl UpdateTarget {
    /// Returns the captured running version, not untrusted release metadata.
    pub fn version(&self) -> &Version {
        match self {
            Self::Package(installed) => installed.version(),
            Self::WindowsMsi(installed) => installed.version(),
        }
    }

    /// Returns the native platform captured by local qualification.
    pub fn platform(&self) -> Platform {
        match self {
            Self::Package(installed) => installed.platform(),
            Self::WindowsMsi(installed) => installed.platform(),
        }
    }

    /// Returns the sole signed payload kind that can update this target.
    pub const fn kind(&self) -> ArtifactKind {
        match self {
            Self::Package(_) => ArtifactKind::PortableArchive,
            Self::WindowsMsi(_) => ArtifactKind::WindowsMsi,
        }
    }

    /// Returns the host-owned parent for private download and preparation staging.
    ///
    /// Rename-based packages stage on the installation filesystem; Windows
    /// Installer stages privately for its separately elevated transaction.
    pub fn staging_parent(&self) -> &Path {
        match self {
            Self::Package(installed) => installed.directory(),
            Self::WindowsMsi(installed) => installed.staging_parent(),
        }
    }
}
