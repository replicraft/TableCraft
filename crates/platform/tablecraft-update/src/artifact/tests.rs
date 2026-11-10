use super::ArtifactKind;
use crate::{UpdateError, platform::Platform};
use semver::Version;

#[test]
fn exact_release_names_cover_both_architectures_without_cross_format_fallback() {
    let version = Version::new(0, 1, 0);
    for (kind, platform, expected) in [
        (
            ArtifactKind::PortableArchive,
            Platform::WindowsX64,
            "tablecraft-desktop-0.1.0-windows-x64.zip",
        ),
        (
            ArtifactKind::PortableArchive,
            Platform::WindowsArm64,
            "tablecraft-desktop-0.1.0-windows-arm64.zip",
        ),
        (
            ArtifactKind::PortableArchive,
            Platform::MacosX64,
            "tablecraft-desktop-0.1.0-macos-x64.zip",
        ),
        (
            ArtifactKind::PortableArchive,
            Platform::MacosArm64,
            "tablecraft-desktop-0.1.0-macos-arm64.zip",
        ),
        (
            ArtifactKind::PortableArchive,
            Platform::LinuxX64,
            "tablecraft-desktop-0.1.0-linux-x64.tar.gz",
        ),
        (
            ArtifactKind::PortableArchive,
            Platform::LinuxArm64,
            "tablecraft-desktop-0.1.0-linux-arm64.tar.gz",
        ),
        (
            ArtifactKind::AppImage,
            Platform::LinuxX64,
            "tablecraft-desktop-0.1.0-linux-x64.AppImage",
        ),
        (
            ArtifactKind::AppImage,
            Platform::LinuxArm64,
            "tablecraft-desktop-0.1.0-linux-arm64.AppImage",
        ),
        (
            ArtifactKind::WindowsMsi,
            Platform::WindowsX64,
            "tablecraft-desktop-0.1.0-windows-x64.msi",
        ),
        (
            ArtifactKind::WindowsMsi,
            Platform::WindowsArm64,
            "tablecraft-desktop-0.1.0-windows-arm64.msi",
        ),
    ] {
        assert_eq!(kind.file_name(&version, platform).unwrap(), expected);
    }
    for (kind, platform) in [
        (ArtifactKind::AppImage, Platform::WindowsX64),
        (ArtifactKind::AppImage, Platform::MacosArm64),
        (ArtifactKind::WindowsMsi, Platform::LinuxX64),
        (ArtifactKind::WindowsMsi, Platform::MacosX64),
    ] {
        assert!(matches!(
            kind.file_name(&version, platform),
            Err(UpdateError::ReleaseMetadata)
        ));
    }
}
