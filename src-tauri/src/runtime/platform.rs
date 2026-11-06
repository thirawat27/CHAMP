//! The OS and CPU a runtime package is built for.

/// Platform information
#[expect(
    dead_code,
    reason = "Cross-platform variants are selected by cfg on their target OS/architecture"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    WindowsX64,
    WindowsArm64,
    MacOSX64,
    MacOSArm64,
    LinuxX64,
    LinuxArm64,
}

impl Platform {
    /// Detect the current platform
    #[allow(unreachable_code)]
    pub fn current() -> Self {
        #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
        return Platform::WindowsX64;

        #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
        return Platform::WindowsArm64;

        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        return Platform::MacOSX64;

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        return Platform::MacOSArm64;

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        return Platform::LinuxX64;

        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        return Platform::LinuxArm64;

        // Default to Linux x64 for unknown platforms
        Platform::LinuxX64
    }

    /// Get the URL key for config lookup (matches JSON keys)
    pub fn url_key(&self) -> String {
        match self {
            Platform::WindowsX64 => "windowsX64",
            Platform::WindowsArm64 => "windowsArm64",
            Platform::LinuxX64 => "linuxX64",
            Platform::LinuxArm64 => "linuxArm64",
            Platform::MacOSX64 => "macOSX64",
            Platform::MacOSArm64 => "macOSArm64",
        }
        .to_string()
    }
}
