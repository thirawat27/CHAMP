//! System Dependency Checker
//!
//! This module checks for required system libraries and dependencies
//! needed by the runtime binaries (especially MySQL/MariaDB on Linux).

use serde::{Deserialize, Serialize};
#[cfg(target_os = "linux")]
use std::path::Path;

/// Represents a system dependency that needs to be checked
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    /// Name of the library (e.g., "libaio")
    pub name: String,
    /// Whether the dependency is installed/available
    pub installed: bool,
    /// Human-readable description
    pub description: String,
    /// Install command(s) for various distributions
    pub install_commands: Vec<InstallCommand>,
}

/// Install command for a specific Linux distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallCommand {
    /// Distribution name (e.g., "Ubuntu/Debian", "Fedora", "Arch Linux")
    pub distribution: String,
    /// Command to install the dependency
    pub command: String,
}

/// Result of dependency check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCheckResult {
    /// All checked dependencies
    pub dependencies: Vec<Dependency>,
    /// Whether all dependencies are satisfied
    pub all_satisfied: bool,
    /// Platform-specific notes
    pub platform_notes: String,
}

/// Check system dependencies for the current platform
pub fn check_system_dependencies() -> DependencyCheckResult {
    let dependencies = Vec::new();

    #[cfg(target_os = "linux")]
    {
        // Using the official MySQL generic package. Additional dependency checks are
        // handled by process startup errors until CHAMP has per-distro runtime packages.
    }

    #[cfg(target_os = "windows")]
    {
        // Windows typically has all required dependencies bundled
        // No additional checks needed
    }

    #[cfg(target_os = "macos")]
    {
        // macOS typically has all required dependencies
        // No additional checks needed
    }

    let all_satisfied = dependencies.iter().all(|d: &Dependency| d.installed);
    let platform_notes = get_platform_notes();

    DependencyCheckResult {
        dependencies,
        all_satisfied,
        platform_notes,
    }
}

/// Check for libaio library on Linux
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn check_libaio() -> Dependency {
    // Check for libaio.so.1 specifically (what MySQL needs)
    let has_symlink = check_library("libaio.so.1");

    // Also check if the actual library file exists
    let has_lib_file = check_library("libaio.so.1.0.1") || check_library("libaio.so.1.0.0");

    // It's only properly installed if the symlink exists
    let installed = has_symlink;

    // Build install commands with symlink fix instructions
    let mut install_commands = vec![
        InstallCommand {
            distribution: "Ubuntu 24.04+ / Debian 12+".to_string(),
            command: "sudo apt install libaio1t64 && sudo ln -sf /usr/lib/x86_64-linux-gnu/libaio.so.1.0.1 /usr/lib/x86_64-linux-gnu/libaio.so.1".to_string(),
        },
        InstallCommand {
            distribution: "Ubuntu/Debian (older)".to_string(),
            command: "sudo apt install libaio1".to_string(),
        },
        InstallCommand {
            distribution: "Fedora/RHEL/CentOS".to_string(),
            command: "sudo dnf install libaio".to_string(),
        },
        InstallCommand {
            distribution: "Arch Linux".to_string(),
            command: "sudo pacman -S libaio".to_string(),
        },
        InstallCommand {
            distribution: "openSUSE".to_string(),
            command: "sudo zypper install libaio1".to_string(),
        },
    ];

    // If the library file exists but symlink doesn't, add a fix command
    if has_lib_file && !has_symlink {
        let symlink_path = find_libaio_path();
        if let Some(base_path) = symlink_path {
            install_commands.insert(
                0,
                InstallCommand {
                    distribution: "Fix: Create missing symlink".to_string(),
                    command: format!(
                        "sudo ln -sf {} {}",
                        base_path,
                        get_symlink_target(&base_path)
                    ),
                },
            );
        }
    }

    Dependency {
        name: "libaio".to_string(),
        installed,
        description: "Linux AIO library - Required by MySQL/MariaDB".to_string(),
        install_commands,
    }
}

/// Find the path to the libaio library file
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn find_libaio_path() -> Option<String> {
    let paths = [
        "/usr/lib/x86_64-linux-gnu/libaio.so.1.0.1",
        "/usr/lib/x86_64-linux-gnu/libaio.so.1.0.0",
        "/usr/lib64/libaio.so.1.0.1",
        "/usr/lib64/libaio.so.1.0.0",
        "/usr/lib/libaio.so.1.0.1",
        "/usr/lib/libaio.so.1.0.0",
        "/lib/x86_64-linux-gnu/libaio.so.1.0.1",
        "/lib/x86_64-linux-gnu/libaio.so.1.0.0",
    ];

    for path in paths {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    None
}

/// Get the target path for the symlink based on the library path
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn get_symlink_target(lib_path: &str) -> String {
    // Extract the directory and create the symlink path
    if let Some(parent) = Path::new(lib_path).parent() {
        let symlink_path = parent.join("libaio.so.1");
        return symlink_path.to_string_lossy().to_string();
    }

    // Fallback to common location
    "/usr/lib/x86_64-linux-gnu/libaio.so.1".to_string()
}

/// Check if a shared library is available on the system
#[cfg(target_os = "linux")]
#[allow(dead_code)]
fn check_library(lib_name: &str) -> bool {
    // Try to find the library using common paths
    let paths = [
        "/lib",
        "/lib64",
        "/usr/lib",
        "/usr/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
    ];

    for base in paths {
        let path = Path::new(base).join(lib_name);
        if path.exists() {
            return true;
        }
    }

    // Also try using ldconfig if available
    if let Ok(output) = std::process::Command::new("ldconfig").arg("-p").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains(lib_name) {
            return true;
        }
    }

    false
}

/// Get platform-specific notes
fn get_platform_notes() -> String {
    #[cfg(target_os = "linux")]
    {
        "Using the official MySQL generic package. If startup fails, check the MySQL log for missing system libraries.".to_string()
    }

    #[cfg(target_os = "windows")]
    {
        "All dependencies are bundled with the application.".to_string()
    }

    #[cfg(target_os = "macos")]
    {
        "All dependencies are bundled with the application.".to_string()
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        "Unknown platform. Please check documentation.".to_string()
    }
}
