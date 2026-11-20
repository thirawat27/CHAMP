//! Pinned downloads for the optional services.
//!
//! Every archive is pinned to an exact release and its SHA-256 (taken from the
//! GitHub release asset digests), so an add-on is never installed unverified.
//! To update one, change the version, URL, and checksum together.
//!
//! Redis and Memcached publish no official Windows builds; CHAMP uses the
//! community builds most Windows stacks ship. On macOS and Linux CHAMP uses a
//! `redis-server` / `memcached` already installed by the system package manager.

use super::AddonKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchiveFormat {
    Zip,
    TarGz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AddonDownload {
    pub url: &'static str,
    pub sha256: &'static str,
    pub format: ArchiveFormat,
    /// The server executable inside the archive, with `/` separators.
    pub binary: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AddonPackage {
    pub version: &'static str,
    /// `None`: no portable build for this platform; use a system install.
    pub download: Option<AddonDownload>,
}

const REDIS_VERSION: &str = "8.10.3";
const MEMCACHED_VERSION: &str = "1.6.8";
const MAILPIT_VERSION: &str = "1.34.0";

/// Windows on ARM runs these x64 builds through its built-in emulation.
const REDIS_WINDOWS_X64: AddonDownload = AddonDownload {
    url: "https://github.com/redis-windows/redis-windows/releases/download/8.10.3/Redis-8.10.3-Windows-x64-msys2.zip",
    sha256: "af16df340525b431e9ec8dabfd625d5e7a233f82805b5418819904ea909c447f",
    format: ArchiveFormat::Zip,
    binary: "Redis-8.10.3-Windows-x64-msys2/redis-server.exe",
};

const MEMCACHED_WINDOWS_X64: AddonDownload = AddonDownload {
    url: "https://github.com/jefyt/memcached-windows/releases/download/1.6.8_mingw_libressl/memcached-1.6.8-win64-mingw.zip",
    sha256: "48ec62cef718f0d73698414b783c0e4a69821013553ca00afe0eed324eb5994b",
    format: ArchiveFormat::Zip,
    binary: "memcached-1.6.8-win64-mingw/bin/memcached.exe",
};

fn mailpit_download(os: &str, arch: &str) -> Option<AddonDownload> {
    let (url, sha256, format, binary) = match (os, arch) {
        ("windows", "x86_64") => (
            "https://github.com/axllent/mailpit/releases/download/v1.34.0/mailpit-windows-amd64.zip",
            "8f080d41374bf22686606374a0a483c8563c1bf29ae078179792392ff8c911d7",
            ArchiveFormat::Zip,
            "mailpit.exe",
        ),
        ("windows", "aarch64") => (
            "https://github.com/axllent/mailpit/releases/download/v1.34.0/mailpit-windows-arm64.zip",
            "59ccb08fb30660e9f0e3f8a721abc32a4aca88ba37cea004ec69940245d8554e",
            ArchiveFormat::Zip,
            "mailpit.exe",
        ),
        ("linux", "x86_64") => (
            "https://github.com/axllent/mailpit/releases/download/v1.34.0/mailpit-linux-amd64.tar.gz",
            "af236b42f171b1c74f0ac5ff444f44d36f2f2ecc0134f5a2f1d4e307e56a0d2c",
            ArchiveFormat::TarGz,
            "mailpit",
        ),
        ("linux", "aarch64") => (
            "https://github.com/axllent/mailpit/releases/download/v1.34.0/mailpit-linux-arm64.tar.gz",
            "179f3fe9642edb3a123edc613003e45641869c5b2728c2076c7bef1c3e09dd01",
            ArchiveFormat::TarGz,
            "mailpit",
        ),
        ("macos", "x86_64") => (
            "https://github.com/axllent/mailpit/releases/download/v1.34.0/mailpit-darwin-amd64.tar.gz",
            "a497582d4e9f9a5d0eea84f8622530533dce0797cf8a4952bd7a59487e5d9c7c",
            ArchiveFormat::TarGz,
            "mailpit",
        ),
        ("macos", "aarch64") => (
            "https://github.com/axllent/mailpit/releases/download/v1.34.0/mailpit-darwin-arm64.tar.gz",
            "32933e0bd17e9738f19ab2eb5ec4d50e183448c9f29ba97e762d418a0f72e854",
            ArchiveFormat::TarGz,
            "mailpit",
        ),
        _ => return None,
    };
    Some(AddonDownload {
        url,
        sha256,
        format,
        binary,
    })
}

/// The package CHAMP installs for `kind` on the given OS/arch
/// (`std::env::consts` values).
pub(super) fn package_for(kind: AddonKind, os: &str, arch: &str) -> AddonPackage {
    let windows = os == "windows" && matches!(arch, "x86_64" | "aarch64");
    match kind {
        AddonKind::Redis => AddonPackage {
            version: REDIS_VERSION,
            download: windows.then_some(REDIS_WINDOWS_X64),
        },
        AddonKind::Memcached => AddonPackage {
            version: MEMCACHED_VERSION,
            download: windows.then_some(MEMCACHED_WINDOWS_X64),
        },
        AddonKind::Mailpit => AddonPackage {
            version: MAILPIT_VERSION,
            download: mailpit_download(os, arch),
        },
    }
}

pub(super) fn current_package(kind: AddonKind) -> AddonPackage {
    package_for(kind, std::env::consts::OS, std::env::consts::ARCH)
}

/// What to tell the user when there is no portable build to download.
pub(super) fn system_install_hint(kind: AddonKind, os: &str) -> Option<&'static str> {
    match (kind, os) {
        (AddonKind::Redis, "macos") => Some("brew install redis"),
        (AddonKind::Redis, "linux") => {
            Some("sudo apt install redis-server  (or your distro's redis package)")
        }
        (AddonKind::Memcached, "macos") => Some("brew install memcached"),
        (AddonKind::Memcached, "linux") => {
            Some("sudo apt install memcached  (or your distro's memcached package)")
        }
        _ => None,
    }
}
