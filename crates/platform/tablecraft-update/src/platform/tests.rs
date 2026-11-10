use super::Platform;
use semver::Version;

#[test]
fn archive_names_match_the_portable_desktop_packaging_contract() {
    let version = Version::parse("0.1.0").unwrap();
    let cases = [
        (
            Platform::WindowsX64,
            "x86_64-pc-windows-msvc",
            "tablecraft-desktop-0.1.0-windows-x64.zip",
        ),
        (
            Platform::WindowsArm64,
            "aarch64-pc-windows-msvc",
            "tablecraft-desktop-0.1.0-windows-arm64.zip",
        ),
        (
            Platform::MacosX64,
            "x86_64-apple-darwin",
            "tablecraft-desktop-0.1.0-macos-x64.zip",
        ),
        (
            Platform::MacosArm64,
            "aarch64-apple-darwin",
            "tablecraft-desktop-0.1.0-macos-arm64.zip",
        ),
        (
            Platform::LinuxX64,
            "x86_64-unknown-linux-gnu",
            "tablecraft-desktop-0.1.0-linux-x64.tar.gz",
        ),
        (
            Platform::LinuxArm64,
            "aarch64-unknown-linux-gnu",
            "tablecraft-desktop-0.1.0-linux-arm64.tar.gz",
        ),
    ];

    for (platform, target, archive) in cases {
        assert_eq!(platform.target(), target);
        assert_eq!(platform.asset_name(&version), archive);
    }
}

#[test]
fn native_detection_does_not_claim_another_architecture_or_abi() {
    if let Some(platform) = Platform::detect() {
        let target = platform.target();
        let architecture = std::env::consts::ARCH;
        assert!(target.starts_with(architecture));

        match std::env::consts::OS {
            "windows" => assert!(target.ends_with("windows-msvc")),
            "macos" => assert!(target.ends_with("apple-darwin")),
            "linux" => assert!(target.ends_with("linux-gnu")),
            other => panic!("unexpected supported host: {other}"),
        }
    }
}
