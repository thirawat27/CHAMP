//! Pinned downloads for the optional services.
//!
//! Every archive is pinned to an exact release and its SHA-256 (taken from the
//! GitHub release asset digests), so an add-on is never installed unverified.
//! To update one, change the version, URL, and checksum together.
//!
//! Redis publishes no official Windows build; CHAMP uses the community build
//! most Windows stacks ship. On macOS and Linux CHAMP uses a `redis-server`
//! already installed by the system package manager. Mailpit and Meilisearch
//! publish official builds for every platform.

use super::AddonKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArchiveFormat {
    Zip,
    TarGz,
    /// The download is the executable itself (Meilisearch).
    Binary,
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
const MAILPIT_VERSION: &str = "1.34.0";
/// 1.56.2 shipped without an Apple Silicon build, so 1.56.1 is pinned.
const MEILISEARCH_VERSION: &str = "1.56.1";

/// Windows on ARM runs these x64 builds through its built-in emulation.
const REDIS_WINDOWS_X64: AddonDownload = AddonDownload {
    url: "https://github.com/redis-windows/redis-windows/releases/download/8.10.3/Redis-8.10.3-Windows-x64-msys2.zip",
    sha256: "af16df340525b431e9ec8dabfd625d5e7a233f82805b5418819904ea909c447f",
    format: ArchiveFormat::Zip,
    binary: "Redis-8.10.3-Windows-x64-msys2/redis-server.exe",
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

fn meilisearch_download(os: &str, arch: &str) -> Option<AddonDownload> {
    let (url, sha256, binary) = match (os, arch) {
        // Windows on ARM runs the x64 build through its built-in emulation.
        ("windows", "x86_64" | "aarch64") => (
            "https://github.com/meilisearch/meilisearch/releases/download/v1.56.1/meilisearch-windows-amd64.exe",
            "ac6d2506cafe862c93a6add533e125a19852339ec21303413120bde84aad3a48",
            "meilisearch.exe",
        ),
        ("linux", "x86_64") => (
            "https://github.com/meilisearch/meilisearch/releases/download/v1.56.1/meilisearch-linux-amd64",
            "b2c7a647d4cdfdd7a901466bfd6797077b849e90f5eb2c0f5b1d05869592e27e",
            "meilisearch",
        ),
        ("linux", "aarch64") => (
            "https://github.com/meilisearch/meilisearch/releases/download/v1.56.1/meilisearch-linux-aarch64",
            "f73987f92b094fd15b79956f9bfc9e8fc7294ad6ad610ebf8a4403c170890726",
            "meilisearch",
        ),
        ("macos", "x86_64") => (
            "https://github.com/meilisearch/meilisearch/releases/download/v1.56.1/meilisearch-macos-amd64",
            "cfd3d4214df99a0270227924cedb62f4db6a857c2ec26723a33b3086f59c3985",
            "meilisearch",
        ),
        ("macos", "aarch64") => (
            "https://github.com/meilisearch/meilisearch/releases/download/v1.56.1/meilisearch-macos-apple-silicon",
            "293b48304c8452cf67c5ab3f5882e542bfe63d52f357bb155f576412cb6f739d",
            "meilisearch",
        ),
        _ => return None,
    };
    Some(AddonDownload {
        url,
        sha256,
        format: ArchiveFormat::Binary,
        binary,
    })
}

/// 6.10.3 is the newest release that is at least two weeks old.
const CENTRIFUGO_VERSION: &str = "6.10.3";
/// 1.0.5 and 1.0.6 were under two weeks old when this was pinned.
const RUSTFS_VERSION: &str = "1.0.4";

fn centrifugo_download(os: &str, arch: &str) -> Option<AddonDownload> {
    let (url, sha256, format, binary) = match (os, arch) {
        // Windows on ARM runs the x64 build through its built-in emulation.
        ("windows", "x86_64" | "aarch64") => (
            "https://github.com/centrifugal/centrifugo/releases/download/v6.10.3/centrifugo_6.10.3_windows_amd64.zip",
            "794300b83b76b81738acf87d77d2a3c191e548a26496d4bbad78f5399a523066",
            ArchiveFormat::Zip,
            "centrifugo.exe",
        ),
        ("linux", "x86_64") => (
            "https://github.com/centrifugal/centrifugo/releases/download/v6.10.3/centrifugo_6.10.3_linux_amd64.tar.gz",
            "780bfd4d1266746c3a8bbc66d2ffe66d7bac8917bfa9218b46c83771a1e669d5",
            ArchiveFormat::TarGz,
            "centrifugo",
        ),
        ("linux", "aarch64") => (
            "https://github.com/centrifugal/centrifugo/releases/download/v6.10.3/centrifugo_6.10.3_linux_arm64.tar.gz",
            "d332630f8aeaf045845206ab74061c77ee0ac8746d788743959c59764ca22071",
            ArchiveFormat::TarGz,
            "centrifugo",
        ),
        ("macos", "x86_64") => (
            "https://github.com/centrifugal/centrifugo/releases/download/v6.10.3/centrifugo_6.10.3_darwin_amd64.tar.gz",
            "d1424c17821e41b0aaebf9a054b0cdd7982d7d4674c22b2db0435000872ff2d3",
            ArchiveFormat::TarGz,
            "centrifugo",
        ),
        ("macos", "aarch64") => (
            "https://github.com/centrifugal/centrifugo/releases/download/v6.10.3/centrifugo_6.10.3_darwin_arm64.tar.gz",
            "8a6854615b8296027966b88d73bf5576a084002b40fc46c4e82afb458cdbe78f",
            ArchiveFormat::TarGz,
            "centrifugo",
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

/// RustFS has no Intel Mac build; musl builds run on any Linux.
fn rustfs_download(os: &str, arch: &str) -> Option<AddonDownload> {
    let (url, sha256, binary) = match (os, arch) {
        ("windows", "x86_64" | "aarch64") => (
            "https://github.com/rustfs/rustfs/releases/download/1.0.4/rustfs-windows-x86_64-v1.0.4.zip",
            "2e3af0b187dd70435dfdefe5f61aae7eede5a21fc9a2a2f07e289f727adbcfa6",
            "rustfs.exe",
        ),
        ("linux", "x86_64") => (
            "https://github.com/rustfs/rustfs/releases/download/1.0.4/rustfs-linux-x86_64-musl-v1.0.4.zip",
            "4924ad4b699b9f59943051efedae2d1eb6d4cac503a1b0384c4b25f7c2c31b5c",
            "rustfs",
        ),
        ("linux", "aarch64") => (
            "https://github.com/rustfs/rustfs/releases/download/1.0.4/rustfs-linux-aarch64-musl-v1.0.4.zip",
            "81414fd0e41969f4f5c2e0555951cee76d3f224c112c269c80f52b31276f187a",
            "rustfs",
        ),
        ("macos", "aarch64") => (
            "https://github.com/rustfs/rustfs/releases/download/1.0.4/rustfs-macos-aarch64-v1.0.4.zip",
            "49057482769d7904ef95dc40457d71122f48cbb8e9e0725a0817a3ec6daf8270",
            "rustfs",
        ),
        _ => return None,
    };
    Some(AddonDownload {
        url,
        sha256,
        format: ArchiveFormat::Zip,
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
        AddonKind::Mailpit => AddonPackage {
            version: MAILPIT_VERSION,
            download: mailpit_download(os, arch),
        },
        AddonKind::Meilisearch => AddonPackage {
            version: MEILISEARCH_VERSION,
            download: meilisearch_download(os, arch),
        },
        AddonKind::Centrifugo => AddonPackage {
            version: CENTRIFUGO_VERSION,
            download: centrifugo_download(os, arch),
        },
        AddonKind::Rustfs => AddonPackage {
            version: RUSTFS_VERSION,
            download: rustfs_download(os, arch),
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
        _ => None,
    }
}
