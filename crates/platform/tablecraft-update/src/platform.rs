//! Native platform identities and the desktop archive names they select.

/// A native desktop platform with a matching portable release archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    /// Windows on x86-64.
    WindowsX64,
    /// Windows on ARM64.
    WindowsArm64,
    /// macOS on Intel processors.
    MacosX64,
    /// macOS on Apple Silicon.
    MacosArm64,
    /// Linux on x86-64.
    LinuxX64,
    /// Linux on ARM64.
    LinuxArm64,
}

impl Platform {
    /// Detects a supported native build target without inspecting the filesystem.
    ///
    /// Recognizing a platform does not establish that the installation can be
    /// replaced. Package-manager and development installations need a separate
    /// installation-policy decision.
    pub const fn detect() -> Option<Self> {
        if cfg!(all(
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )) {
            Some(Self::WindowsX64)
        } else if cfg!(all(
            target_os = "windows",
            target_arch = "aarch64",
            target_env = "msvc"
        )) {
            Some(Self::WindowsArm64)
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            Some(Self::MacosX64)
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Some(Self::MacosArm64)
        } else if cfg!(all(
            target_os = "linux",
            target_arch = "x86_64",
            target_env = "gnu"
        )) {
            Some(Self::LinuxX64)
        } else if cfg!(all(
            target_os = "linux",
            target_arch = "aarch64",
            target_env = "gnu"
        )) {
            Some(Self::LinuxArm64)
        } else {
            None
        }
    }

    /// Returns the Rust target whose desktop archive this platform consumes.
    pub const fn target(self) -> &'static str {
        match self {
            Self::WindowsX64 => "x86_64-pc-windows-msvc",
            Self::WindowsArm64 => "aarch64-pc-windows-msvc",
            Self::MacosX64 => "x86_64-apple-darwin",
            Self::MacosArm64 => "aarch64-apple-darwin",
            Self::LinuxX64 => "x86_64-unknown-linux-gnu",
            Self::LinuxArm64 => "aarch64-unknown-linux-gnu",
        }
    }
}

#[cfg(test)]
mod tests;
