//! Exact signed release artifact identities selected by installation kind.

use crate::{UpdateError, platform::Platform};
use semver::Version;

const REPOSITORY_URL: &str = "https://github.com/replicraft/TableCraft";

/// The signed payload format consumed by a qualified installation route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactKind {
    /// A portable directory or macOS application-bundle archive.
    PortableArchive,
    /// A complete Linux Type 2 AppImage, replaced as one authenticated file.
    AppImage,
    /// A native Windows Installer package applied through Windows Installer.
    WindowsMsi,
}

impl ArtifactKind {
    /// Returns the exact release filename for a supported kind/platform pair.
    ///
    /// # Errors
    /// Refuses AppImages for non-Linux targets and MSI packages for non-Windows targets.
    pub fn file_name(self, version: &Version, platform: Platform) -> Result<String, UpdateError> {
        let (target, portable_extension) = match platform {
            Platform::WindowsX64 => ("windows-x64", "zip"),
            Platform::WindowsArm64 => ("windows-arm64", "zip"),
            Platform::MacosX64 => ("macos-x64", "zip"),
            Platform::MacosArm64 => ("macos-arm64", "zip"),
            Platform::LinuxX64 => ("linux-x64", "tar.gz"),
            Platform::LinuxArm64 => ("linux-arm64", "tar.gz"),
        };
        let extension = match self {
            Self::PortableArchive => portable_extension,
            Self::AppImage if matches!(platform, Platform::LinuxX64 | Platform::LinuxArm64) => {
                "AppImage"
            }
            Self::WindowsMsi
                if matches!(platform, Platform::WindowsX64 | Platform::WindowsArm64) =>
            {
                "msi"
            }
            Self::AppImage | Self::WindowsMsi => return Err(UpdateError::ReleaseMetadata),
        };
        Ok(format!("tablecraft-desktop-{version}-{target}.{extension}"))
    }
}

pub(crate) fn download_url(version: &Version, file_name: &str) -> String {
    format!("{REPOSITORY_URL}/releases/download/v{version}/{file_name}")
}

#[cfg(test)]
mod tests;
